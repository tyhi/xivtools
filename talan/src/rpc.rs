use crate::config;
use crate::craft;
use crate::macros::Macro;
use crate::recipe;
use crate::task;
use anyhow::{anyhow, Context, Error};
use std::fmt;
use std::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub enum Request {
    Recipe {
        item: String,
        job: Option<u32>,
        count: u32,
    },
    Macros(Vec<Macro>),
    Craft {
        options: config::Options,
        tasks: Vec<task::Task>,
    },

    StopCrafting,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Request::Recipe { item, .. } => write!(f, "Request::Recipe({})", item),
            Request::Macros(_) => write!(f, "Request::Macros"),
            Request::Craft { .. } => write!(f, "Request::Craft"),
            Request::StopCrafting => write!(f, "Request::StopCrafting"),
        }
    }
}

#[derive(Debug)]
pub enum Response {
    Error(String),
    Recipe {
        recipe: Option<recipe::Recipe>,
        count: u32,
    },
    Craft(Vec<task::Status>),
    Idle, // End of Work, aka finished.
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Response::Error(s) => write!(f, "Response::Error(\"{}\")", s),
            Response::Recipe { recipe, .. } => write!(f, "Response::Recipe({})", recipe.is_some()),
            Response::Craft(_) => write!(f, "Response::Craft"),
            Response::Idle => write!(f, "Response::Idle"),
        }
    }
}

pub struct Worker {
    rx: Receiver<Request>,
    tx: Sender<Response>,
    macros: Vec<Macro>,
}

impl Worker {
    pub fn new(rx: Receiver<Request>, tx: Sender<Response>) -> Self {
        Self {
            rx,
            tx,
            macros: Vec::new(),
        }
    }

    fn try_receive(&self) -> Option<Request> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            _ => None,
        }
    }

    fn receive(&self) -> Option<Request> {
        match self.rx.recv() {
            Ok(req) => {
                log::debug!("worker <= {}", req);
                Some(req)
            }
            Err(e) => {
                log::error!("worker <= failed to receive request: {}", e.to_string());
                None
            }
        }
    }

    fn reply(&self, resp: Response) -> Result<(), Error> {
        log::debug!("worker => {}", resp);
        self.tx
            .send(resp)
            .context("worker => failed to send response")
    }

    pub fn worker_thread(&mut self) -> Result<(), Error> {
        log::trace!("worker thread started");
        loop {
            if let Some(request) = self.receive() {
                let result = match request {
                    Request::Recipe { item, job, count } => {
                        log::trace!("querying xivapi for \"{}\" (job: {:?})", item, job);
                        let recipe_result = if let Ok(search_results) = xivapi::query_recipe(&item)
                        {
                            recipe::Recipe::filter(&search_results, &item, job)
                        } else {
                            None
                        };
                        log::trace!("query result: {:#?}", recipe_result);
                        self.reply(Response::Recipe {
                            recipe: recipe_result,
                            count,
                        })
                    }
                    Request::Macros(macros) => {
                        self.macros = macros;
                        Ok(())
                    }
                    Request::Craft { options, tasks } => {
                        self.craft_request(options, &tasks, &self.macros[..])
                    }
                    unknown => Err(anyhow!("Unexpected RPC received: {:?}", unknown)),
                };

                if let Err(e) = result {
                    self.reply(Response::Error(e.to_string()))?;
                }
            }
        }
    }

    fn craft_request(
        &self,
        options: config::Options,
        tasks: &[task::Task],
        macros: &[Macro],
    ) -> Result<(), Error> {
        // Send a full status update to the main thread after completing
        // an item.
        let status_fn = |status: &[task::Status]| -> Result<(), Error> {
            self.reply(Response::Craft(status.to_vec()))
        };

        // Check whether crafting should continue after each craft.
        let continue_fn = || -> bool {
            if let Some(Request::StopCrafting) = self.try_receive() {
                return false;
            }
            true
        };

        // If init throws an error we'll have a log to console anyway.
        let handle = xiv::init()?;
        let craft = craft::Crafter::new(handle, &options, macros, tasks, status_fn, continue_fn);

        // TODO: Do something useful with errors here.
        craft?.craft_items()?;
        self.reply(Response::Idle)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::{Error, Result};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::thread;

    fn setup() -> (Sender<Request>, Receiver<Response>) {
        let (client_tx, worker_rx): (Sender<Request>, Receiver<Request>) = channel();
        let (worker_tx, client_rx): (Sender<Response>, Receiver<Response>) = channel();
        thread::spawn(move || Worker::new(worker_rx, worker_tx).worker_thread());

        (client_tx, client_rx)
    }

    // Test that a simple Request -> Response cycle works with the worker thread
    // in the background using a sample recipe. This tests both having the job
    // specified and not.
    #[test]
    fn worker_recipe_test() -> Result<(), Error> {
        let (tx, rx) = setup();
        let item1 = "Rakshasa Axe";
        tx.send(Request::Recipe {
            item: item1.to_string(),
            job: None,
            count: 1,
        })?;

        match rx.recv()? {
            Response::Recipe { recipe, count } => {
                let r = recipe.unwrap();
                assert_eq!(r.name, item1);
                assert_eq!(r.job, 1); // BSM
                assert_eq!(r.index, 0);
                assert_eq!(count, 1);
            }
            _ => panic!("unexpected response"),
        }

        let item2 = "Cloud Pearl";
        tx.send(Request::Recipe {
            item: item2.to_string(),
            job: Some(5),
            count: 3,
        })?;

        match rx.recv()? {
            Response::Recipe { recipe, count } => {
                let r = recipe.unwrap();
                assert_eq!(r.name, item2);
                assert_eq!(r.job, 5); // WVR
                assert_eq!(r.index, 11);
                assert_eq!(count, 3);
            }
            _ => panic!("unexpected response"),
        }
        Ok(())
    }

    // Ensure that we can queue up recipes + count tuples to the worker and
    // receive them on the other end in the proper order and count to create a
    // task list.
    #[test]
    fn worker_recipe_list_test() -> Result<(), Error> {
        let (tx, rx) = setup();
        let recipe_list = vec![
            ("Cloud Pearl", 1),
            ("Prismatic Ingot", 2),
            ("Rakshasa Axe", 3),
            ("White Ash Lumber", 1),
        ];
        let mut tasks: Vec<task::Task> = Vec::new();
        for (item, count) in &recipe_list {
            tx.send(Request::Recipe {
                item: item.to_string(),
                count: *count,
                job: None,
            })
            .unwrap();
        }

        for _ in 0..recipe_list.len() {
            let resp = rx.recv().unwrap();
            match resp {
                Response::Recipe { recipe, count } => {
                    tasks.push(task::Task::new(recipe.unwrap(), count));
                }
                _ => panic!("unexpected response"),
            }
        }

        for i in 0..recipe_list.len() {
            assert_eq!(recipe_list[i].0, tasks[i].recipe.name);
            assert_eq!(recipe_list[i].1, tasks[i].quantity);
        }

        Ok(())
    }
}
