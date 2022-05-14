use crate::action::{Action, Direction};
use crate::input;

use nanorand::{ChaCha, Rng};
use std::cmp::Ordering;
use std::f32::consts;
use std::fmt::{self, Display, Formatter};
use std::io::{Error, Write};
use std::ops::AddAssign;

#[cfg(debug_assertions)]
const DEBUG: bool = true;
#[cfg(not(debug_assertions))]
const DEBUG: bool = false;

#[derive(Debug)]
pub struct Context<W: Write> {
    w: W,
    rng: ChaCha<20>,
    last_desc: String,
    loc: Location,
    time: Time,
    season: Season,
    sky: Sky,
    wind: Wind,
    temp: i8, // celcius
}

impl<W: Write> Context<W> {
    pub fn spawn(write: W) -> Self {
        let loc = Location::Forest(Coord::new()); // TODO: randomize coords
        let time = Time::new(6, 0); // TODO: randomize time
        let season = Season::Winter;
        let sky = Sky::Rain;
        let wind = Wind::High;

        let mut ctx = Self {
            w: write,
            rng: ChaCha::new(),
            last_desc: String::new(),
            loc,
            temp: loc.temp_base(season, time, sky),
            time,
            season,
            sky,
            wind,
        };
        ctx.last_desc = ctx.to_string();

        ctx
    }

    pub fn available_actions(&self) -> Vec<Action> {
        [Action::Describe, Action::Go, Action::Wait, Action::Quit]
            .into_iter()
            .collect()
    }

    pub fn available_directions(&self) -> Vec<Direction> {
        let mut dirs = Vec::new();

        dirs.push(Direction::North);
        dirs.push(Direction::South);
        dirs.push(Direction::East);
        dirs.push(Direction::West);

        dirs
    }

    pub fn act(&mut self, action: Action) -> Result<bool, Error> {
        match action {
            Action::Describe => {
                let description = self.to_string();
                write!(self.w, "{}", description)?;
                self.last_desc = description.to_string();
            }

            Action::Go => {
                let directions = self.available_directions();
                match self.loc {
                    Location::Forest(ref mut coord) => {
                        writeln!(self.w, "which direction?")?;
                        let direction = input::menu(&mut self.w, &directions)?;
                        *coord += direction.as_coord_with_magnitude(1);
                        writeln!(self.w, "you head {}.", direction)?;
                        self.time_tick(0, 1)?;
                    }
                }
            }

            Action::Wait => {
                writeln!(self.w, "some time passes.")?; // TODO: mix up time pass messages
                self.time_tick(0, 5)?;
            }

            Action::Quit => return Ok(true),
        }

        if self.description_changed() {
            writeln!(self.w, "your surroundings look different.")?;
            writeln!(self.w)?;
            self.act(Action::Describe)?;
        }

        Ok(false)
    }

    fn time_tick(&mut self, hours: u8, mins: u8) -> Result<(), Error> {
        self.time.tick(hours, mins);
        if DEBUG {
            writeln!(
                self.w,
                "debug: {} {}C ({}C)",
                self.time,
                self.temp,
                self.loc.temp_base(self.season, self.time, self.sky)
            )?;
        }

        let total_mins: u64 = (u16::from(hours) * Time::HOUR_MINS + u16::from(mins)).into();
        let mut new_temp = self.temp;
        for _ in 0..total_mins {
            // give temperature chance to change
            if self.rng.generate_range(0_u32..=100_000) < self.loc.chance_temp_change() {
                // generate temperature change
                // HACK: nanorand doesn't do this as expected with signed ints, so have to offset by 1
                // (THIS IS A WORKAROUND FOR A BUG IN NANORAND)
                let mut delta: i8 = self.rng.generate_range(
                    1..=self.loc.temp_max_change(self.season, self.time, self.sky) + 1,
                );
                assert!(!delta.is_negative());

                let toward_base: bool =
                    self.rng.generate_range(0_u32..=100_000) < self.loc.chance_temp_toward_base();

                // move temp toward or away from base
                match self
                    .temp
                    .cmp(&self.loc.temp_base(self.season, self.time, self.sky))
                {
                    Ordering::Less => {
                        if !toward_base {
                            // temp . . . base
                            // <- away from base
                            delta = delta.saturating_neg();
                        }
                    }

                    Ordering::Greater => {
                        if toward_base {
                            // base . . . temp
                            // <- toward base
                            delta = delta.saturating_neg();
                        }
                    }

                    Ordering::Equal => {
                        // temperature is currently at base
                        // equal chance to move above or below base
                        if self.rng.generate::<bool>() {
                            delta = delta.saturating_neg();
                        }
                    }
                }

                new_temp = self.temp.saturating_add(delta);
            }

            // give wind chance to change
            if self.rng.generate_range(0_u32..=100_000) < self.loc.chance_wind_change() {
                if self.rng.generate_range(0_u32..=100_000) < self.loc.chance_wind_increase() {
                    // increase wind
                    if self.wind.increase() {
                        writeln!(self.w, "the wind speeds up.")?;
                    };
                } else {
                    // decrease wind
                    if self.wind.decrease() {
                        writeln!(self.w, "the wind slows down.")?;
                    };
                }
            }

            /* give sky chance to change */
            for (chance, new_sky) in self.loc.chances_sky() {
                if self.rng.generate_range(0_u32..=100_000) < chance {
                    match (self.sky, new_sky, self.temp < 0) {
                        (Sky::Clear, Sky::Clear, _) => (),
                        (Sky::Clouds, Sky::Clouds, _) => (),
                        (Sky::Rain, Sky::Rain, _) => (),

                        (Sky::Clear, Sky::Clouds, _) => writeln!(self.w, "it gets cloudy.")?,
                        (Sky::Clear | Sky::Clouds, Sky::Rain, true) => {
                            writeln!(self.w, "it starts snowing.")?;
                        }
                        (Sky::Clear | Sky::Clouds, Sky::Rain, false) => {
                            writeln!(self.w, "it starts raining.")?;
                        }
                        (Sky::Clouds | Sky::Rain, Sky::Clear, _) => {
                            writeln!(self.w, "the sky clears up.")?;
                        }
                        (Sky::Rain, Sky::Clouds, true) => writeln!(self.w, "it stops snowing.")?,
                        (Sky::Rain, Sky::Clouds, false) => writeln!(self.w, "it stops raining.")?,
                    }

                    self.sky = new_sky;
                }
            }
        }

        // notice temperature changes
        match new_temp.cmp(&self.temp) {
            Ordering::Less => writeln!(self.w, "it feels colder.")?,
            Ordering::Greater => writeln!(self.w, "it feels warmer.")?,
            Ordering::Equal => (),
        }
        self.temp = new_temp;

        Ok(())
    }

    fn description_changed(&self) -> bool {
        self.to_string() != self.last_desc
    }
}

impl<W: Write> Display for Context<W> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        // where are we?
        match self.loc {
            Location::Forest(_) => writeln!(f, "you are in a forest.")?,
        };

        // is it cloudy?
        if self.sky == Sky::Clouds {
            writeln!(f, "it is cloudy.")?;
        }

        // what's the air like?
        match (self.wind, self.sky, TempCat::classify(self.temp)) {
            (
                Wind::None,
                Sky::Clear | Sky::Clouds,
                temp @ (TempCat::Freezing | TempCat::Chilly),
            ) => {
                writeln!(f, "it is {}.", temp)?;
            }
            (Wind::None, Sky::Clear | Sky::Clouds, TempCat::Neutral) => {
                writeln!(f, "the air is still.")?;
            }
            (Wind::None, Sky::Clear | Sky::Clouds, temp @ (TempCat::Warm | TempCat::Hot)) => {
                writeln!(f, "the air is {} and still.", temp)?;
            }
            (Wind::None, Sky::Rain, TempCat::Freezing) => writeln!(f, "it is snowing.")?,
            (Wind::None, Sky::Rain, TempCat::Hot) => writeln!(f, "it is hot and rainy.")?,
            (
                Wind::None | Wind::Light,
                Sky::Rain,
                TempCat::Chilly | TempCat::Neutral | TempCat::Warm,
            ) => writeln!(f, "it is raining.")?,
            (Wind::Light, Sky::Rain, TempCat::Freezing) => {
                writeln!(f, "it is snowing with a frigid breeze.")?;
            }
            (Wind::Light, Sky::Rain, TempCat::Hot) => {
                writeln!(f, "it is raining with a hot breeze.")?;
            }
            (Wind::Light, Sky::Clear | Sky::Clouds, temp) => {
                writeln!(f, "there is a {} breeze.", temp)?;
            }
            (Wind::Medium, Sky::Clear | Sky::Clouds, TempCat::Freezing) => {
                writeln!(f, "there is a bitter wind.")?;
            }
            (Wind::Medium, Sky::Clear | Sky::Clouds, temp) => {
                writeln!(f, "there is a {} wind.", temp)?;
            }
            (Wind::Medium, Sky::Rain, TempCat::Freezing) => {
                writeln!(f, "it is snowing with a bitter wind.")?;
            }
            (Wind::Medium, Sky::Rain, TempCat::Chilly | TempCat::Neutral | TempCat::Warm) => {
                writeln!(f, "it is raining and windy.")?;
            }
            (Wind::Medium, Sky::Rain, TempCat::Hot) => {
                writeln!(f, "there are hot gusts of rain.")?;
            }
            (Wind::High, Sky::Clear | Sky::Clouds, TempCat::Freezing) => {
                writeln!(f, "the wind howls and bites.")?;
            }
            (Wind::High, Sky::Clear | Sky::Clouds, TempCat::Chilly | TempCat::Neutral) => {
                writeln!(f, "the wind howls.")?
            }
            (Wind::High, Sky::Clear | Sky::Clouds, temp @ (TempCat::Warm | TempCat::Hot)) => {
                writeln!(f, "there are strong gusts of {} wind.", temp)?
            }
            (Wind::High, Sky::Rain, TempCat::Freezing) => {
                writeln!(f, "the wind howls and bites. it is snowing furiously.")?;
            }
            (Wind::High, Sky::Rain, TempCat::Chilly | TempCat::Neutral | TempCat::Warm) => {
                writeln!(f, "it is raining furiously.")?;
            }
            (Wind::High, Sky::Rain, TempCat::Hot) => {
                writeln!(f, "the hot rain blows furiously.")?;
            }
        }

        // what's the time of day? we might have very little to go off of.
        match (self.time.classify(self.season), self.sky) {
            (TimeCat::Dawn, Sky::Clear) => writeln!(f, "the sun is rising.")?,
            (TimeCat::Dusk, Sky::Clear) => writeln!(f, "the sun is setting.")?,
            (TimeCat::Dawn | TimeCat::Dusk, _) => writeln!(f, "the sky is dark grey.")?,

            (TimeCat::Morning, Sky::Clear) => writeln!(f, "it is a clear morning.")?,
            (TimeCat::Noon, Sky::Clear) => writeln!(f, "it is midday.")?,
            (TimeCat::Afternoon, Sky::Clear) => writeln!(f, "it is the afternoon.")?,
            (TimeCat::Morning | TimeCat::Noon | TimeCat::Afternoon, _) => {
                writeln!(f, "the sky is grey.")?;
            }

            (TimeCat::Night, _) => writeln!(f, "it is dark.")?,
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Location {
    Forest(Coord),
}

impl Location {
    fn sunlight(&self, season: Season, time: Time, sky: Sky) -> f32 {
        let sky_sun = season.sunlight_level(time);

        let sun_intensity = match season {
            Season::Spring => 0.80,
            Season::Summer => 1.00,
            Season::Autumn => 0.90,
            Season::Winter => 0.70,
        };

        let sky_visibility = (match self {
            Self::Forest(_) => match season {
                Season::Spring => 0.8,
                Season::Summer => 0.6,
                Season::Autumn => 0.7,
                Season::Winter => 0.9,
            },
        }) * (match sky {
            Sky::Clear => 1.0,
            Sky::Clouds => 0.7,
            Sky::Rain => 0.6,
        });

        sky_sun * sun_intensity * sky_visibility
    }
}

#[allow(clippy::zero_prefixed_literal)]
impl Location {
    pub fn temp_base(&self, season: Season, time: Time, sky: Sky) -> i8 {
        let base = match self {
            Self::Forest(_) => match season {
                Season::Spring => 0,
                Season::Summer => 6,
                Season::Autumn => 9,
                Season::Winter => -5,
            },
        };

        const DIURNAL_VAR: f32 = 10.0;
        let sun_bias = (self.sunlight(season, time, sky) - 0.5) * DIURNAL_VAR * 2.0;
        base + sun_bias as i8
    }

    pub fn temp_max_change(&self, season: Season, time: Time, sky: Sky) -> i8 {
        const MAX_CHANGE: f32 = 4.0;
        // NOTE: should be positive, otherwise toward base chance is flipped
        (self.sunlight(season, time, sky) * MAX_CHANGE) as i8 + 1
    }

    pub const fn chance_temp_toward_base(&self) -> u32 {
        match self {
            Self::Forest(_) => 60_000,
        }
    }

    pub const fn chance_temp_change(&self) -> u32 {
        match self {
            Self::Forest(_) => 16_667, // 1 change / 10 mins
        }
    }

    pub const fn chance_wind_change(&self) -> u32 {
        match self {
            Self::Forest(_) => 1_667, // 1 change / 1 hr
        }
    }

    pub const fn chance_wind_increase(&self) -> u32 {
        match self {
            Self::Forest(_) => 50_000,
        }
    }

    pub const fn chances_sky(&self) -> [(u32, Sky); 3] {
        match self {
            Self::Forest(_) => [
                (0_208, Sky::Clear),  // 1 change / 8 hr
                (0_417, Sky::Clouds), // 1 change / 4 hrs
                (0_139, Sky::Rain),   // 1 change / 12 hrs
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coord {
    pub n: i8, // north
    pub w: i8, // west
}

impl Coord {
    pub const fn new() -> Self {
        Self { n: 0, w: 0 }
    }
}

impl AddAssign for Coord {
    fn add_assign(&mut self, other: Coord) {
        self.n = self.n.saturating_add(other.n);
        self.w = self.w.saturating_add(other.w);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Time {
    // NOTE: never exceeds Self::DAY_MINS
    mins: u16,
}

impl Time {
    pub const HOUR_MINS: u16 = 60;
    pub const DAY_HOURS: u16 = 24;
    pub const DAY_MINS: u16 = Self::HOUR_MINS * Self::DAY_HOURS;

    pub const fn new(hour: u8, min: u8) -> Self {
        Self {
            mins: (hour as u16 * Self::HOUR_MINS) + min as u16,
        }
    }

    pub fn get(&self) -> (u8, u8) {
        let hour = self.mins / Self::HOUR_MINS;
        let min = self.mins % Self::HOUR_MINS;
        (hour as u8, min as u8)
    }

    pub fn tick(&mut self, hours: u8, mins: u8) {
        for _ in 0..hours {
            self.mins += Self::HOUR_MINS;
            self.wrap_mins();
        }

        self.mins += u16::from(mins);
        self.wrap_mins();
    }

    pub fn classify(&self, season: Season) -> TimeCat {
        let (sunrise, sunset) = season.sunlight_times();

        if self.mins < sunrise.mins || self.mins > sunset.mins {
            TimeCat::Night
        } else {
            // stretch the sunset/sunrise times so at 0 is dawn, and 255 is dusk.
            let stretch = f32::from(self.mins - sunrise.mins)
                / f32::from(sunset.mins - sunrise.mins)
                * f32::from(u8::MAX);
            match stretch as u8 {
                0..=31 => TimeCat::Dawn,
                32..=111 => TimeCat::Morning,
                112..=143 => TimeCat::Noon,
                144..=223 => TimeCat::Afternoon,
                224..=255 => TimeCat::Dusk,
            }
        }
    }

    // uphold self.mins invariant
    fn wrap_mins(&mut self) {
        self.mins %= Self::DAY_MINS;
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let (hours, mins) = self.get();
        write!(f, "{:0>2}:{:0>2}", hours, mins)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeCat {
    Dawn,
    Morning,
    Noon,
    Afternoon,
    Dusk,
    Night,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn sunlight_times(&self) -> (Time, Time) {
        match self {
            Season::Spring => (Time::new(8, 0), Time::new(17, 0)),
            Season::Summer => (Time::new(4, 0), Time::new(22, 30)),
            Season::Autumn => (Time::new(5, 0), Time::new(21, 30)),
            Season::Winter => (Time::new(9, 0), Time::new(15, 0)),
        }
    }

    pub fn sunlight_level(&self, time: Time) -> f32 {
        let (sunrise, sunset) = self.sunlight_times();

        if time.mins < sunrise.mins || time.mins > sunset.mins {
            0.0
        } else {
            ((f32::from(time.mins - sunrise.mins) * consts::PI)
                / f32::from(sunset.mins - sunrise.mins))
            .sin()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sky {
    Clear,
    Clouds,
    Rain,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wind {
    None,
    Light,
    Medium,
    High,
}

impl Wind {
    pub fn increase(&mut self) -> bool {
        let old = *self;
        let new = match old {
            Self::None => Self::Light,
            Self::Light => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::High,
        };

        *self = new;
        old != new
    }

    pub fn decrease(&mut self) -> bool {
        let old = *self;
        let new = match old {
            Self::None => Self::None,
            Self::Light => Self::None,
            Self::Medium => Self::Light,
            Self::High => Self::Medium,
        };

        *self = new;
        old != new
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum TempCat {
    Freezing,
    Chilly,
    Neutral,
    Warm,
    Hot,
}

impl TempCat {
    pub fn classify(temp: i8) -> Self {
        match temp {
            i8::MIN..=0 => Self::Freezing,
            1..=19 => Self::Chilly,
            20..=25 => Self::Neutral,
            26..=31 => Self::Warm,
            32..=i8::MAX => Self::Hot,
        }
    }
}

impl Display for TempCat {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let adjective = match self {
            Self::Freezing => "frigid",
            Self::Chilly => "chilly",
            Self::Neutral => "light",
            Self::Warm => "warm",
            Self::Hot => "hot",
        };
        write!(f, "{}", adjective)
    }
}
