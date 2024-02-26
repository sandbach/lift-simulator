use rand::seq::SliceRandom;
use rand::thread_rng;
use rand::Rng;
use ratatui::widgets::{Bar, BarGroup};
use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
    vec::Vec,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Passenger {
    pub from_floor: i32,
    pub to_floor: i32,
    riding: bool,
}

impl Passenger {
    pub fn new(from_floor: i32, to_floor: i32) -> Passenger {
        Passenger {
            from_floor,
            to_floor,
            riding: false,
        }
    }
}

#[derive(Debug)]
pub struct Building {
    pub bottom_floor: i32,
    pub top_floor: i32,
    lifts: Vec<Arc<Lift>>,
}

impl Building {
    pub fn new(bottom_floor: i32, top_floor: i32, lift_count: u32) -> Building {
        let lifts = sequence(lift_count)
            .iter()
            .map(|x| Arc::new(Lift::new(*x)))
            .collect();
        start_threads(&lifts);
        Building {
            bottom_floor,
            top_floor,
            lifts,
        }
    }

    pub fn lift_count(&self) -> u16 {
        self.lifts.len() as u16
    }

    fn abs_floor(&self, floor: i32) -> u64 {
        let value = floor - self.bottom_floor;
        if value < 0 {
            return 0;
        }
        value as u64
    }

    pub fn max_value(&self) -> u64 {
        difference(self.bottom_floor, self.top_floor) as u64
    }

    pub fn data(&self) -> Result<BarGroup, String> {
        let mut bars = Vec::new();
        for lift in &self.lifts {
            let (floor, _, _) = lift.get_info()?;
            let label = lift.label()?;
            // Bar::default().value(10).label("e".into())
            bars.push(
                Bar::default()
                    .value(self.abs_floor(floor))
                    .label(label.into()),
            );
        }
        Ok(BarGroup::default().bars(bars.as_slice()))
    }

    // pub fn info(&self) -> Result<Vec<(String, u64)>, String> {
    //     let mut output = Vec::new();
    //     for lift in &self.lifts {
    //         let (floor, _, _) = lift.get_info()?;
    //         let label = lift.label()?;
    //         output.push((label.into(), self.abs_floor(floor)));
    //     }
    //     Ok(output)
    // }

    pub fn respond(&self, passenger: Passenger) -> Result<usize, String> {
        if let Ok(index) = self.best_lift(&passenger) {
            self.lifts[index].add_passenger(passenger)?;
            return Ok(index);
        }
        Err(format!("Could not respond to passenger: {:?}.", passenger))
    }

    pub fn random(&self) {
        let mut floors: Vec<i32> = (self.bottom_floor..self.top_floor).collect();
        floors.shuffle(&mut thread_rng());
        // let from = floors.pop().unwrap();
        // let to = floors.pop().unwrap();
        let _ = self.respond(Passenger::new(floors[0], floors[1]));
    }

    pub fn realistic_random(&self) {
        let mut rng = rand::thread_rng();
        let rand = rng.gen_range(self.bottom_floor..self.top_floor);
        let mut floors = vec![0, rand];
        floors.shuffle(&mut thread_rng());
        let _ = self.respond(Passenger::new(floors[0], floors[1]));
    }

    fn best_lift(&self, passenger: &Passenger) -> Result<usize, String> {
        let mut best = 0;
        let mut closest = i32::MAX;
        let lifts = &self.lifts;
        let mut indices: Vec<usize> = (0..lifts.len()).collect();
        indices.shuffle(&mut thread_rng());
        for index in indices {
            let lift = &lifts[index];
            if let Ok(dist) = lift.distance_from(passenger) {
                if dist < closest {
                    closest = dist;
                    best = index;
                }
            }
        }
        Ok(best)
    }

    pub fn debug(&self) {
        eprintln!("{:?}", self);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Stopped,
}

#[derive(Debug)]
struct Lift {
    id: RwLock<u32>,
    floor: RwLock<i32>,
    direction: RwLock<Direction>,
    doors_open: RwLock<bool>,
    passengers: RwLock<Vec<Passenger>>,
    targets: RwLock<Vec<i32>>,
}

const MS_PER_FLOOR: u64 = 500;
const DOOR_OPEN_TIME: u64 = 750;

impl Lift {
    fn new(id: u32) -> Lift {
        Lift {
            id: RwLock::new(id),
            floor: RwLock::new(0),
            direction: RwLock::new(Direction::Stopped),
            doors_open: RwLock::new(false),
            passengers: RwLock::new(Vec::new()),
            targets: RwLock::new(Vec::new()),
        }
    }

    fn get_info(&self) -> Result<(i32, Direction, bool), String> {
        let floor = *self
            .floor
            .read()
            .map_err(|e| format!("Failed to read-lock floor: {}", e))?;
        let direction = *self
            .direction
            .read()
            .map_err(|e| format!("Failed to read-lock direction: {}", e))?;
        let doors_open = *self
            .doors_open
            .read()
            .map_err(|e| format!("Failed to read-lock doors_open: {}", e))?;
        Ok((floor, direction, doors_open))
    }

    fn move_towards(&self, target: i32) -> Result<(i32, Direction, bool), String> {
        // let id = *self
        //     .id
        //     .read()
        //     .map_err(|e| format!("Failed to read-lock id: {}", e))?;
        let (floor, direction, _) = self.get_info()?;
        if target > floor {
            self.set_direction(Direction::Up)?;
        } else if target < floor {
            self.set_direction(Direction::Down)?;
        }
        // println!("Lift {}: On floor {}, going to {}.", id, floor, target);
        wait_millis(MS_PER_FLOOR);
        match direction {
            Direction::Up => self.reach_floor(floor + 1)?,
            Direction::Down => self.reach_floor(floor - 1)?,
            Direction::Stopped => self.reach_floor(floor)?,
        };
        // if let Direction::Up = direction {
        //     self.reach_floor(floor + 1)?;
        // } else  {
        //     self.reach_floor(floor - 1)?;
        // }
        self.get_info()
    }

    fn set_floor(&self, new_floor: i32) -> Result<(i32, Direction, bool), String> {
        let mut floor = self
            .floor
            .write()
            .map_err(|e| format!("Failed to write-lock direction: {}", e))?;
        *floor = new_floor;
        drop(floor);
        self.get_info()
    }

    fn set_direction(&self, new_direction: Direction) -> Result<(i32, Direction, bool), String> {
        let mut direction = self
            .direction
            .write()
            .map_err(|e| format!("Failed to write-lock direction: {}", e))?;
        *direction = new_direction;
        drop(direction);
        self.get_info()
    }

    fn set_doors_open(&self, status: bool) -> Result<(i32, Direction, bool), String> {
        let mut doors_open = self
            .doors_open
            .write()
            .map_err(|e| format!("Failed to write-lock doors_opening: {}", e))?;
        *doors_open = status;
        drop(doors_open);
        self.get_info()
    }

    fn open_doors(&self) -> Result<(i32, Direction, bool), String> {
        // let id = *self
        //     .id
        //     .read()
        //     .map_err(|e| format!("Failed to read-lock id: {}", e))?;
        // let (floor, _) = self.get_info()?;
        // println!("Lift {}: Doors opening on floor {}.", id, floor);
        self.set_doors_open(true)?;
        wait_millis(DOOR_OPEN_TIME);
        // println!("Lift {}: Doors closing on floor {}.", id, floor);
        wait_millis(DOOR_OPEN_TIME);
        self.set_doors_open(false)?;
        self.get_info()
    }

    fn add_target(&self, target: i32) -> Result<(i32, Direction, bool), String> {
        let mut targets = self
            .targets
            .write()
            .map_err(|e| format!("Failed to write-lock targets: {}", e))?;
        binary_add(&mut targets, target);
        drop(targets);
        self.get_info()
    }

    fn reach_floor(&self, new_floor: i32) -> Result<(i32, Direction, bool), String> {
        self.set_floor(new_floor)?;
        let mut passengers = self
            .passengers
            .write()
            .map_err(|e| format!("Failed to write-lock passengers: {}", e))?;
        let mut targets = self
            .targets
            .write()
            .map_err(|e| format!("Failed to write-lock targets: {}", e))?;
        let mut open_doors = false;
        if let Ok(pos) = targets.binary_search(&new_floor) {
            targets.remove(pos);
            open_doors = true;
        }
        drop(targets);
        let mut to_remove: Vec<usize> = vec![];
        for i in 0..passengers.len() {
            let passenger = &mut passengers[i];
            if passenger.from_floor == new_floor {
                passenger.riding = true;
                self.add_target(passenger.to_floor)?;
            }
            if passenger.to_floor == new_floor && passenger.riding {
                to_remove.push(i);
            }
        }
        for i in to_remove.iter().rev() {
            passengers.remove(*i);
        }
        drop(passengers);
        if open_doors {
            self.open_doors()?;
        }
        self.get_info()
    }

    fn add_passenger(&self, passenger: Passenger) -> Result<(i32, Direction, bool), String> {
        let mut passengers = self
            .passengers
            .write()
            .map_err(|e| format!("Failed to write-lock passengers: {}", e))?;
        binary_add(&mut passengers, passenger);
        self.add_target(passenger.from_floor)?;
        drop(passengers);
        self.get_info()
    }

    fn next_target(&self) -> Result<i32, String> {
        let targets = self
            .targets
            .read()
            .map_err(|e| format!("Failed to read-lock targets: {}", e))?;
        if targets.is_empty() {
            return Err(format!("There are no more targets."));
        }
        let (floor, direction, _) = self.get_info()?;
        let pos = match targets.binary_search(&floor) {
            // Ok(x) => return Ok(targets[x]),
            Ok(x) => {
                if direction == Direction::Down && x == 0 {
                    self.set_direction(Direction::Up)?;
                } else if direction == Direction::Up && x == targets.len() {
                    self.set_direction(Direction::Down)?;
                }
                return Ok(targets[x]);
            }
            Err(x) => x,
        };
        if pos == targets.len() {
            self.set_direction(Direction::Down)?;
            return Ok(targets[pos - 1]);
        } else if pos == 0 {
            self.set_direction(Direction::Up)?;
            return Ok(targets[0]);
        } else if direction == Direction::Up {
            return Ok(targets[pos]);
        } else {
            return Ok(targets[pos - 1]);
        }
    }

    fn distance_from(&self, passenger: &Passenger) -> Result<i32, String> {
        let p_floor = passenger.from_floor;
        let p_dir = if passenger.to_floor > p_floor {
            Direction::Up
        } else {
            Direction::Down
        };
        let (l_floor, l_dir, _) = self.get_info()?;
        let targets = self
            .targets
            .read()
            .map_err(|e| format!("Failed to read-lock targets: {}", e))?;
        if l_dir == Direction::Stopped || targets.is_empty() {
            return Ok(difference(l_floor, p_floor));
        }
        if (l_dir == Direction::Down && p_dir == Direction::Down && l_floor > p_floor)
            || (l_dir == Direction::Up && p_dir == Direction::Up && l_floor < p_floor)
        {
            return Ok(difference(l_floor, p_floor));
        }
        let last_target = if l_dir == Direction::Up {
            targets[targets.len() - 1]
        } else {
            targets[0]
        };
        let distance = difference(l_floor, last_target) + difference(last_target, p_floor);
        Ok(distance)
    }

    fn label(&self) -> Result<String, String> {
        let (floor, direction, doors_open) = self.get_info()?;
        let mut symbol = match direction {
            Direction::Up => '↑',
            Direction::Down => '↓',
            Direction::Stopped => ' ',
        };
        if doors_open {
            symbol = '↔';
            // return Ok(format!("{} ↔", floor))
        }
        Ok(format!("{} {}", floor, symbol))
    }
}

fn sequence(n: u32) -> Vec<u32> {
    (0..n).collect()
}

fn wait_millis(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

fn difference(x: i32, y: i32) -> i32 {
    if x > y {
        x - y
    } else {
        y - x
    }
}

fn start_threads(lifts: &Vec<Arc<Lift>>) {
    for lift in lifts {
        let arc = Arc::clone(lift);
        thread::spawn(move || -> Result<(i32, Direction, bool), String> {
            loop {
                if let Ok(target) = arc.next_target() {
                    arc.move_towards(target)?;
                } else {
                    arc.set_direction(Direction::Stopped)?;
                }
                wait_millis(100);
            }
        });
    }
}

fn binary_add<T: Ord>(vec: &mut Vec<T>, item: T) {
    if let Err(pos) = vec.binary_search(&item) {
        vec.insert(pos, item);
    }
}

#[cfg(test)]
mod tests {
    use crate::building::*;

    #[test]
    fn difference_check() {
        assert_eq!(difference(10, 10), 0);
        assert_eq!(difference(100, 10), 90);
        assert_eq!(difference(10, 100), 90);
        assert_eq!(difference(-2, 3), 5);
    }
}
