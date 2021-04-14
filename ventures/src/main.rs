use anyhow::{anyhow, Error, Result};
use chrono::Local;
use chrono::TimeZone;
use console::style;
use simple_logger::SimpleLogger;
use std::thread;
use std::time::Duration;
use structopt::StructOpt;
use xiv::{retainer, ui, CityState, ClassJob, Process};

#[derive(Debug, StructOpt)]
#[structopt(name = "ventures", about = "A FFXIV venture automation helper")]
struct Opts {
    /// Use slower menu navigation for slower or laggier systems
    #[structopt(short = "s", long = "slow")]
    use_slow_navigation: bool,

    /// Enable log levels.
    #[structopt(short = "v", parse(from_occurrences))]
    verbose: u64,

    #[structopt(short = "p")]
    print_retainers: bool,
}

fn parse_arguments() -> Result<(xiv::XivHandle, bool), Error> {
    let args = Opts::from_args();
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_module_level(
            "ventures",
            match args.verbose {
                0 => log::LevelFilter::Info,
                1 => log::LevelFilter::Debug,
                _ => log::LevelFilter::Trace,
            },
        )
        .init()?;

    let mut h = xiv::init()?;
    h.use_slow_navigation = args.use_slow_navigation;

    Ok((h, args.print_retainers))
}
fn print_retainers(retainers: &[retainer::Retainer]) {
    println!(
        "{:24} {:10} {:20} {:^6} {:32}",
        "Name", "Class/Job", "Home", "Active", "Venture"
    );
    for retainer in retainers.iter() {
        if retainer.is_valid() {
            println!(
                "{:24} {:10} {:20} {:^6} {:32}",
                style(retainer.name()).cyan(),
                style(format!(
                    "{} {}",
                    retainer.level,
                    ClassJob::from(retainer.classjob)
                ))
                .yellow(),
                style(CityState::from(retainer.home_city)).magenta(),
                if retainer.available {
                    style("\u{2713}").green()
                } else {
                    style("x").red()
                },
                style(retainer.venture()).blue()
            );
        }
    }
}

fn main() -> Result<(), Error> {
    let proc = Process::new("ffxiv_dx11.exe")?;
    let (hnd, args_print_retainers) = parse_arguments()?;
    let mut retainer_tbl = retainer::RetainerState::new(proc, retainer::OFFSET);
    retainer_tbl.read()?;
    if retainer_tbl.count == 0 {
        return Err(anyhow!(
            "No retainers found! Try opening a retainer bell at least once after logging in."
        ));
    }

    if args_print_retainers {
        print_retainers(&retainer_tbl.retainers);
        return Ok(());
    }

    let mut display_order: Vec<usize> = vec![0; retainer_tbl.display_order.len()];
    let mut print_next = true;
    loop {
        let mut menu_open = false;
        retainer_tbl.read()?;

        // Invert the display order -> retainer array to be retainer -> display
        // order. Calculate this each time we wake up in case the user changed
        // the layout.
        for (pos, r_id) in retainer_tbl.display_order.iter().enumerate() {
            if retainer_tbl.retainers[pos].is_valid() {
                display_order[*r_id as usize] = pos;
            }
        }

        // Cache the retainer state so we can freely do process reads without reference ownership.
        let r_cache = retainer_tbl.retainers;
        for (rdx, (retainer, pos)) in r_cache.iter().zip(display_order.iter()).enumerate() {
            if !retainer.employed() {
                continue;
            }

            let now = Local::now().timestamp();
            let done = retainer.venture_complete;
            if done <= now as u32 {
                let mut updated = false;
                let mut retries = 5;
                // If for some reason we don't update the venture (Lag, UI state, etc) then clear the window and try this retainer again.
                while !updated && retries > 0 {
                    log::info!(
                        "re-assigning {} for {}",
                        retainer.venture(),
                        retainer.name()
                    );
                    if !menu_open {
                        open_retainer_menu(hnd);
                        menu_open = true;
                    }

                    reassign_venture(hnd, *pos);
                    retainer_tbl.read()?;

                    // Did the venture completion date/time change from what we have cached?
                    if retainer_tbl.retainers[rdx].venture_complete == done {
                        log::error!("{}'s venture did not update. Retrying.", retainer.name());
                        menu_open = false;
                        retries -= 1;
                    } else {
                        updated = true;
                    }
                }

                print_next = true;
            }
        }
        if menu_open {
            ui::press_cancel(hnd);
        }

        // Find the next retainer whose venture will be complete, filtering out
        // those who aren't running ventures at all.
        retainer_tbl.read()?;
        let next: retainer::Retainer =
            retainer_tbl
                .retainers
                .iter()
                .fold(retainer_tbl.retainers[0], |acc, r| {
                    if !acc.employed()
                        || (r.employed() && r.venture_complete < acc.venture_complete)
                    {
                        *r
                    } else {
                        acc
                    }
                });

        // It's possible that the user canceled all ventures.
        if next.venture_id == 0 {
            return Err(anyhow!(
                "No retainers were found on any ventures. Try checking the output of -p?"
            ));
        }

        if print_next {
            let next_time = Local.timestamp(i64::from(next.venture_complete), 0);
            log::info!(
                "Next: {}'s \"{}\" will be done at {}",
                next.name(),
                next.venture(),
                next_time.format("%I:%M%P")
            );
            print_next = false;
        }
        thread::sleep(Duration::from_secs(5));
    }
}

fn open_retainer_menu(hnd: xiv::XivHandle) {
    log::debug!("open_retainer_menu");
    // This will close the game menu if open and exit the retainer window if
    // it was open from a previous run.
    ui::press_escape(hnd);
    ui::press_escape(hnd);
    ui::press_cancel(hnd);
    ui::press_cancel(hnd);
    ui::wait(2.0);

    // The reason the menu is opened twice is because we want to clear out any
    // mouse actions the UI registered that would lead to us not having the input
    // cursor up when the retainer menu opens.
    ui::target_nearest_npc(hnd);
    ui::wait(1.0);
    ui::press_confirm(hnd);
    ui::wait(2.0);
    ui::press_cancel(hnd);
    ui::press_cancel(hnd);
    ui::target_nearest_npc(hnd);
    ui::wait(1.0);
    ui::press_confirm(hnd);
    ui::wait(2.0);
}

// General usability rules
// 1. Wait 1 second after moving around in a menu
// 2. Wait 2 seconds after pressing a button for UI changes / Feo Ul / Retainer dialog.
fn reassign_venture(hnd: xiv::XivHandle, r_id: usize) {
    log::debug!("reassign_venture(r_id: {})", r_id);
    for _ in 0..r_id {
        ui::cursor_down(hnd);
    }
    ui::wait(1.0);
    ui::press_confirm(hnd);
    ui::wait(2.0);
    ui::press_confirm(hnd);
    ui::wait(2.0);
    // Move down to Assign Venture / View Venture Progress
    for _ in 0..5 {
        ui::cursor_down(hnd);
    }
    ui::wait(1.0);
    // Select the menu option
    ui::press_confirm(hnd);
    ui::wait(2.0);
    // Move left to 'Reassign'
    ui::cursor_left(hnd);
    ui::wait(1.0);
    // Confirm 'Reassign'
    ui::press_confirm(hnd);
    ui::wait(2.0);
    // Move left to 'Assign' in the venture window that comes up
    ui::cursor_left(hnd);
    ui::wait(1.0);
    // Confirm 'Assign'
    ui::press_confirm(hnd);
    ui::wait(2.0);
    // Confirm the message from the retainer about the venture
    ui::press_confirm(hnd);
    ui::wait(2.0);
    // Escape out of the specific retainer's menu
    ui::press_cancel(hnd);
    ui::wait(2.0);
    // Say goodbye to the retainer
    ui::press_confirm(hnd);
    ui::wait(2.0);
}
