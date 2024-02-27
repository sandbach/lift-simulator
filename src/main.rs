use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Stylize,
    style::{Color, Style},
    widgets::{BarChart, Block, Borders},
    Terminal,
};
use std::io::{stdout, Result};
use clap::Parser;
use tui_textarea::{Input, Key, TextArea};

mod building;

use building::{Building, Passenger};

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_line(percent_x: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_height = 3;
    let popup_perc = (((popup_height as f64) / (r.height as f64)) * (100 as f64)).round() as u16;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - popup_perc) / 2),
            Constraint::Length(popup_height),
            Constraint::Percentage((100 - popup_perc) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}

/// Program to simulate a building with lifts.
///
/// While the program is running, the following keybindings are in effect:
///
/// - <q>: Quit the program.
/// - <space>: Bring up a dialog box to add a new passenger.
/// - <r>: Add a new passenger going between two random floors.
/// - <R>: Add a new passenger going between a random floor and the ground floor.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Bottom floor in the building
    #[arg(short, long, default_value_t = 0)]
    bottom: i32,

    /// Top floor in the building
    #[arg(short, long, default_value_t = 10)]
    top: i32,

    /// Number of lifts in the building
    #[arg(short, long, default_value_t = 5)]
    lifts: u32,
}

#[derive(Debug)]
struct UI<'a> {
    state: UIState,
    from_floor: Option<i32>,
    to_floor: Option<i32>,
    textarea: TextArea<'a>,
}

#[derive(PartialEq, Eq, Debug)]
enum UIState {
    BarChart,
    FromFloorPopup,
    ToFloorPopup,
}

impl UI<'_> {
    fn new(building: &Building) -> UI {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_placeholder_text(format!(
            "Enter a floor number from {} to {}",
            building.bottom_floor, building.top_floor
        ));
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White))
                .title(format!("Going from floor:")),
        );
        UI {
            state: UIState::BarChart,
            from_floor: None,
            to_floor: None,
            textarea,
        }
    }

    fn validate(&mut self, building: &Building) -> bool {
        let mut title = String::new();
        let happy_title = self.popup_title();
        let mut result = false;
        match self.popup_input().parse::<i32>() {
            Err(err) => {
                self.textarea
                    .set_style(Style::default().fg(Color::LightRed));
                title = format!("ERROR: {}", err);
                result = false;
            }
            Ok(val) => {
                if val < building.bottom_floor || val > building.top_floor {
                    self.textarea
                        .set_style(Style::default().fg(Color::LightRed));
                    title = format!(
                        "ERROR: Floor must be between {} and {}.",
                        building.bottom_floor, building.top_floor
                    );
                    result = false;
                } else {
                    title = happy_title.clone();
                    self.textarea.set_style(Style::default().fg(Color::White));
                    result = true;
                }
            }
        }
        if self.textarea.is_empty() {
            title = happy_title.clone();
        }
        self.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White))
                .title(title),
        );
        result
    }

    fn reset(&mut self) {
        self.state = UIState::BarChart;
        self.from_floor = None;
        self.to_floor = None;
        self.clear_input();
    }

    fn clear_input(&mut self) {
        self.textarea.select_all();
        self.textarea.delete_char();
    }

    fn call_lift(&self, building: &Building) {
        building
            .respond(Passenger::new(
                self.from_floor.unwrap(),
                self.to_floor.unwrap(),
            ))
            .unwrap();
    }

    fn popup_title(&self) -> String {
        match self.state {
            UIState::ToFloorPopup => {
                format!("Going from floor {} to:", self.from_floor.unwrap_or(0))
            }
            _ => "Going from floor:".to_string(),
        }
    }

    fn popup_active(&self) -> bool {
        match self.state {
            UIState::FromFloorPopup | UIState::ToFloorPopup => true,
            _ => false,
        }
    }

    fn set_floor(&mut self) {
        let floor = str::parse::<i32>(&self.popup_input()).unwrap();
        match self.from_floor {
            None => self.from_floor = Some(floor),
            Some(_) => self.to_floor = Some(floor),
        }
    }

    // fn set_from_floor(&mut self) {
    //     self.from_floor = Some(str::parse::<i32>(&self.popup_input()).unwrap())
    // }

    // fn set_to_floor(&mut self) {
    //     self.to_floor = Some(str::parse::<i32>(&self.popup_input()).unwrap())
    // }

    fn popup_input(&self) -> String {
        self.textarea.lines()[0].clone()
    }

    // fn parse_input(&self) -> Result<i32, String> {
    //     match str::parse::<i32>(&self.textarea.lines()[0]) {
    //         Ok(val) => Ok(val),
    //         Err(err) => format!("{}", err)
    //     }
    // }

    fn next_state(&mut self) {
        match self.state {
            UIState::BarChart => self.state = UIState::FromFloorPopup,
            UIState::FromFloorPopup => self.state = UIState::ToFloorPopup,
            UIState::ToFloorPopup => self.reset(),
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // let building = Arc::new(Building::new(0, 15, 1));
    let building = Building::new(args.bottom, args.top, args.lifts);

    // let new_build = building.clone();
    // thread::spawn(move || {
    //     new_build.respond(Passenger::new(7, 0)).unwrap();
    //     thread::sleep(Duration::from_millis(1000));
    //     new_build.respond(Passenger::new(4, 1)).unwrap();
    //     thread::sleep(Duration::from_millis(1000));
    //     new_build.respond(Passenger::new(10, -2)).unwrap();
    //     thread::sleep(Duration::from_millis(1000));
    //     new_build.respond(Passenger::new(-4, 0)).unwrap();
    // });
    // thread::spawn(move || {
    //     let mut rng = rand::thread_rng();
    //     loop {
    //         new_build
    //             .respond(Passenger::new(
    //                 rng.gen_range(new_build.bottom_floor..new_build.top_floor),
    //                 rng.gen_range(new_build.bottom_floor..new_build.top_floor),
    //             ))
    //             .unwrap();
    //         thread::sleep(Duration::from_secs(rng.gen_range(1..4)));
    //     }
    // });

    // let layout = Layout::default()
    //     .direction(Direction::Horizontal)
    //     .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref());

    let mut ui = UI::new(&building);
    let mut is_valid = false;
    loop {
        terminal.draw(|frame| {
            let area = frame.size();
            frame.render_widget(
                BarChart::default()
                    .block(Block::default().title("Lifts").borders(Borders::ALL))
                    .bar_width(bar_width(&area, building.lift_count()))
                    .bar_gap(1)
                    .bar_style(Style::new().green().on_blue())
                    .value_style(Style::new().blue().bold())
                    .label_style(Style::new().white())
                    .data(building.data().unwrap())
                    .max(building.max_value()),
                area,
            );

            if ui.popup_active() {
                let popup_area = centered_line(60, frame.size());
                frame.render_widget(ui.textarea.widget(), popup_area);
            }
        })?;
        if event::poll(std::time::Duration::from_millis(16))? {
            if ui.popup_active() {
                match event::read()?.into() {
                    Input { key: Key::Esc, .. } => {
                        ui.reset();
                    }
                    Input {
                        key: Key::Enter, ..
                    } if is_valid => {
                        ui.set_floor();
                        if ui.state == UIState::ToFloorPopup {
                            ui.call_lift(&building);
                        }
                        ui.clear_input();
                        ui.next_state();
                        is_valid = ui.validate(&building);
                    }
                    Input {
                        key: Key::Enter, ..
                    } => {}
                    input => {
                        if ui.textarea.input(input) {
                            is_valid = ui.validate(&building);
                        }
                    }
                }
            } else {
                if let event::Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Esc => break,
                            KeyCode::Char('q') => break,
                            KeyCode::Char(' ') => ui.next_state(),
                            KeyCode::Char('d') => building.debug(),
                            KeyCode::Char('r') => building.random(),
                            KeyCode::Char('R') => building.realistic_random(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}

fn bar_width(rect: &Rect, bars: u16) -> u16 {
    let mut total_width = rect.width;
    total_width -= 2;
    let bar_width = (total_width / bars) - 1;
    if bar_width > 4 {
        bar_width
    } else {
        4
    }
}
