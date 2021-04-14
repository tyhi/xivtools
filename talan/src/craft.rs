use crate::action::Action;
use crate::config::Options;
use crate::macros::Macro;
use crate::task;
use anyhow::{anyhow, Error};
use std::time::{Duration, Instant};

pub struct Crafter<'a, C, S>
where
    C: FnMut() -> bool,
    S: FnMut(&[task::Status]) -> Result<(), Error>,
{
    // TODO: Consolidate xiv::XivHandle and xiv::Process
    handle: xiv::XivHandle,
    game_state: xiv::craft::CraftState,
    options: &'a Options,
    macros: &'a [Macro],
    tasks: &'a [task::Task],
    status_fn: S,
    continue_fn: C,
}

impl<'a, C, S> Crafter<'a, C, S>
where
    C: FnMut() -> bool,
    S: FnMut(&[task::Status]) -> Result<(), Error>,
{
    pub fn new(
        handle: xiv::XivHandle,
        options: &'a Options,
        macros: &'a [Macro],
        tasks: &'a [task::Task],
        status_fn: S,
        continue_fn: C,
    ) -> Result<Self, Error> {
        let proc = xiv::Process::new("ffxiv_dx11.exe")?;
        let game_state = xiv::craft::CraftState::new(proc, xiv::craft::OFFSET);
        // let proc = Process::new("ffxiv_dx11.exe").unwrap();
        // let mut xiv = craft::CraftState::new(&proc, craft::OFFSET);
        // xiv.read().unwrap();
        Ok(Crafter {
            handle,
            game_state,
            options,
            macros,
            tasks,
            status_fn,
            continue_fn,
        })
    }

    // Craft all the configured tasks and update the client by way of |status_callback|.
    pub fn craft_items(&mut self) -> Result<(), Error> {
        self.game_state.read()?;
        // Initialize the crafting status and send an initialize slice
        // so the UI knows what to start rendering.
        let mut status: Vec<task::Status> = self.tasks.iter().map(task::Status::from).collect();
        (self.status_fn)(&status[..])?;

        //handle.use_slow_navigation = options.use_slow_dialog_navigation;
        if self.options.should_clear_window_on_craft {
            // Get the UI into a state we can trust it, and pray the user doesn't touch it.
            xiv::ui::clear_window(self.handle);
        }

        // Clear role actions before we iterate tasks so the game state
        // and role action state will be in sync.
        let mut job: u32 = 256;
        for (i, task) in self.tasks.iter().enumerate() {
            log::trace!("Task: {:?}", task);
            let task_job: usize = task.recipe.job as usize;

            if self.options.gear[task_job] == 0 {
                panic!(
                    "No gear set is configured for {}, aborting tasks!",
                    xiv::JOBS[task_job]
                );
            }

            // Swap our job if necessary. It may have been used in the previous task.
            if job == task.recipe.job {
                log::trace!("already {}, no need to change job.", xiv::JOBS[task_job]);
            } else {
                log::trace!("changing job to {}.", xiv::JOBS[task_job]);
                log::info!("changing to gearset {}", self.options.gear[task_job]);
                xiv::ui::press_enter(self.handle);
                xiv::ui::send_string(
                    self.handle,
                    &format!("/gearset change {}", self.options.gear[task_job]),
                );
                xiv::ui::wait(0.5);
                xiv::ui::press_enter(self.handle);
                // If we don't wait here we might bring the window up before
                // the job has changed, leading to the wrong class seeding the
                // window's mode.
                xiv::ui::wait(1.0);

                job = task.recipe.job;
            }

            // Navigate to the correct recipe based on the index provided
            self.select_recipe(task);
            if !self.options.use_trial_synthesis {
                self.select_materials(task);
            }
            for task_index in 1..=task.quantity {
                log::info!(
                    "crafting {} {}/{}",
                    task.recipe.name,
                    task_index,
                    task.quantity
                );
                // Time to craft the items

                self.execute_task(&self.macros[task.macro_id as usize].actions[..])?;
                status[i].finished += 1;
                (self.status_fn)(&status[..])?;
                // Check if we received a message to stop from the main thread.
                xiv::ui::wait(1.0);
                xiv::ui::press_confirm(self.handle);
            }

            xiv::ui::press_escape(self.handle);
            xiv::ui::wait(2.0);
        }

        Ok(())
    }

    fn open_craft_window(&self) {
        xiv::ui::send_key(self.handle, 'N' as u32);
        xiv::ui::wait(1.0);
    }

    // Selects the appropriate recipe then leaves the cursor on the Synthesize
    // button, ready for material selection.
    fn select_recipe(&self, task: &task::Task) {
        // Bring up the crafting window itself and give it time to appear
        self.open_craft_window();
        log::info!("selecting recipe...");
        // The crafting window always starts with the current job selected and if we press
        // |BACK| 1 more time than the job's index then we will end up at the search box.
        for _ in 0..=task.recipe.job + 1 {
            xiv::ui::cursor_backward(self.handle);
        }
        xiv::ui::press_confirm(self.handle);
        xiv::ui::wait(1.0);
        xiv::ui::send_string(self.handle, &task.recipe.name);
        xiv::ui::press_enter(self.handle);
        xiv::ui::wait(1.0);
        // Navigate to the offset we need
        for _ in 0..task.recipe.index {
            xiv::ui::cursor_down(self.handle);
        }

        // // Select the recipe to get to components / synthesize button
        xiv::ui::press_confirm(self.handle);
    }

    fn select_any_materials(&self, task: &task::Task) {
        // Up to the icon for the bottom material
        xiv::ui::cursor_up(self.handle);
        // Right to the NQ column
        xiv::ui::cursor_right(self.handle);
        // Right to the HQ column
        xiv::ui::cursor_right(self.handle);

        // The cursor should be on the quantity field of the bottom item now
        // We move through the ingredients backwards because we start at the bottom of t
        for (i, material) in task.recipe.mats.iter().rev().enumerate() {
            log::debug!("{}x {}", material.count, material.name);
            for _ in 0..material.count {
                xiv::ui::press_confirm(self.handle)
            }
            // Don't move up if we've made it back to the top of the ingredients
            if i != task.recipe.mats.len() - 1 {
                xiv::ui::cursor_up(self.handle);
            }
        }
        xiv::ui::cursor_left(self.handle);
        for material in &task.recipe.mats {
            for _ in 0..material.count {
                xiv::ui::press_confirm(self.handle)
            }
            xiv::ui::cursor_down(self.handle);
        }
    }

    fn select_materials(&self, task: &task::Task) {
        if !task.specify_materials {
            return self.select_any_materials(task);
        }
        let mut hq_mats = task.mat_quality.iter().fold(0, |acc, &mat| acc + mat.hq);
        // If there are no HQ mats we can fast path this by just
        // starting the synthesis.
        if hq_mats == 0 {
            return;
        }

        // Up to the icon for the bottom material
        xiv::ui::cursor_up(self.handle);
        // Right to the NQ columnu
        xiv::ui::cursor_right(self.handle);
        // Right to the HQ column
        xiv::ui::cursor_right(self.handle);

        // Move up the HQ column and increase the HQ count per the task
        // values. Once there are none left we can shortcut back to the
        // confirm button.
        for (i, mq) in task.mat_quality.iter().rev().enumerate() {
            for _ in 0..mq.hq {
                xiv::ui::press_confirm(self.handle);
            }

            hq_mats -= mq.hq;
            if hq_mats > 0 {
                xiv::ui::cursor_up(self.handle);
            } else {
                for _ in 0..=i {
                    xiv::ui::cursor_down(self.handle);
                }
                break;
            }
        }
    }

    fn wait_for_state(&mut self, expected_states: &[xiv::craft::State]) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            self.game_state.read()?;
            if expected_states.contains(&self.game_state.state) {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Timed out waiting for states {:?} (current state: {})",
            expected_states,
            self.game_state.state
        ))
    }

    // Due to XIV not zeroing out the crafting struct the only way we can be
    // fully sure a crafting step happened is to compare the entire state,
    // otherwise we may end up with situation where a previous run's step
    // matches what we're looking for.
    fn wait_for_step(&mut self, previous_state: &xiv::craft::CraftingStruct) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            self.game_state.read()?;
            if *self.game_state != *previous_state {
                return Ok(());
            }
            std::thread::sleep(Duration::from_micros(100));
        }

        Err(anyhow!("Timed out waiting for the next action to finish"))
    }

    fn execute_task(&mut self, actions: &[&'static Action]) -> Result<(), Error> {
        if self.options.use_trial_synthesis {
            xiv::ui::cursor_left(self.handle);
            xiv::ui::cursor_left(self.handle);
        }
        xiv::ui::press_confirm(self.handle);
        self.wait_for_state(&[xiv::craft::State::READY])?;
        let mut previous_state = *self.game_state;
        for action in actions {
            // If a macro finished early by way of capping progress earlier than
            // expected, or just generally failing by running out of durability
            // then catch it and don't send the rest of the actions.
            if [
                xiv::craft::State::CANCELED,
                xiv::craft::State::FAILED,
                xiv::craft::State::SUCCESS,
            ]
            .iter()
            .any(|&s| s == self.game_state.state)
            {
                break;
            }

            if !(self.continue_fn)() {
                log::trace!("tasks canceled by user");
                return Err(anyhow!("Tasks canceled"));
            }

            xiv::ui::press_enter(self.handle);
            xiv::ui::send_string(self.handle, &format!("/ac \"{}\"", &action.name));
            xiv::ui::press_enter(self.handle);
            self.wait_for_step(&previous_state)?;
            previous_state = *self.game_state;
        }

        // At the end of this sequence the cursor should have selected the recipe
        // again and be on the Synthesize button.
        self.wait_for_state(&[xiv::craft::State::SUCCESS])
    }
}
