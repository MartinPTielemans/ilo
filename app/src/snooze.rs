//! When a snoozed tab comes back (#2350): the short list's four times, the words a time is said
//! in, and what Pick a Date and Time…'s field reads. All on the local clock, from `now`, so the
//! tests can say what each means at a fixed time.

use chrono::{
    DateTime, Datelike, Days, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike,
    Weekday,
};

/// The short list, in its order: 3 hours on, 18:00, 08:00 the next day and 08:00 on Monday
/// (orchestrator's calls on #2350).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    LaterToday,
    ThisEvening,
    TomorrowMorning,
    NextWeek,
}

impl Choice {
    pub const ALL: [Choice; 4] = [
        Choice::LaterToday,
        Choice::ThisEvening,
        Choice::TomorrowMorning,
        Choice::NextWeek,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Choice::LaterToday => "Later Today",
            Choice::ThisEvening => "This Evening",
            Choice::TomorrowMorning => "Tomorrow Morning",
            Choice::NextWeek => "Next Week",
        }
    }

    /// When it means at `now`, or none for This Evening once it's evening, as it would be past or
    /// the same as Later Today.
    pub fn at(self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        let today = now.date();
        match self {
            Choice::LaterToday => Some(to_minute(now + chrono::Duration::hours(3))),
            Choice::ThisEvening => {
                let evening = today.and_time(hm(EVENING));
                (evening > now + chrono::Duration::minutes(30)).then_some(evening)
            }
            Choice::TomorrowMorning => Some(today.succ_opt()?.and_time(hm(MORNING))),
            Choice::NextWeek => {
                let ahead = 7 - today.weekday().num_days_from_monday();
                Some((today + Days::new(ahead.into())).and_time(hm(MORNING)))
            }
        }
    }
}

/// The hour Tomorrow Morning and Next Week come back at, and a day typed without a time.
const MORNING: u32 = 8;
const EVENING: u32 = 18;

fn hm(hour: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, 0, 0).unwrap_or_default()
}

fn to_minute(at: NaiveDateTime) -> NaiveDateTime {
    at.with_second(0)
        .and_then(|at| at.with_nanosecond(0))
        .unwrap_or(at)
}

/// The clock here now, to the minute.
pub fn now() -> NaiveDateTime {
    #[cfg(test)]
    if let Some(now) = TEST_NOW.get() {
        return now;
    }
    to_minute(Local::now().naive_local())
}

#[cfg(test)]
thread_local! {
    static TEST_NOW: std::cell::Cell<Option<NaiveDateTime>> = const { std::cell::Cell::new(None) };
}

/// Stops the clock at `now` for a test's thread, so a menu lists the same times whenever it runs.
#[cfg(test)]
pub fn stop_clock(now: NaiveDateTime) {
    TEST_NOW.set(Some(now));
}

/// `at` on the clock here, in milliseconds since 1970, as the core keeps a snooze's time. A time
/// a clock change skips takes the hour after it.
pub fn millis(at: NaiveDateTime) -> f64 {
    let local = Local.from_local_datetime(&at);
    let time: Option<DateTime<Local>> = local.earliest().or_else(|| {
        Local
            .from_local_datetime(&(at + chrono::Duration::hours(1)))
            .earliest()
    });
    time.map_or(0., |time| time.timestamp_millis() as f64)
}

/// `millis` back on the clock here.
pub fn local(millis: f64) -> Option<NaiveDateTime> {
    Local
        .timestamp_millis_opt(millis as i64)
        .single()
        .map(|time| time.naive_local())
}

/// `at` as a row of the short list ends: "17:25" today, "Wed 08:00" within the week, else with
/// its date, "Mon 24 Nov 08:00".
pub fn short(at: NaiveDateTime, now: NaiveDateTime) -> String {
    let days = (at.date() - now.date()).num_days();
    match days {
        0 => at.format("%H:%M").to_string(),
        1..=6 => at.format("%a %H:%M").to_string(),
        _ => full(at, now),
    }
}

/// `at` with its date, as the picker's line reads it: "Fri 13 Nov 17:30", with the year if it
/// isn't this one.
pub fn full(at: NaiveDateTime, now: NaiveDateTime) -> String {
    match at.year() == now.year() {
        true => at.format("%a %-d %b %H:%M").to_string(),
        false => at.format("%a %-d %b %Y %H:%M").to_string(),
    }
}

/// `at` in a sentence, as the toast says it: "today 17:25", "tomorrow 08:00", "Mon 08:00".
pub fn words(at: NaiveDateTime, now: NaiveDateTime) -> String {
    match (at.date() - now.date()).num_days() {
        0 => format!("today {}", at.format("%H:%M")),
        1 => format!("tomorrow {}", at.format("%H:%M")),
        _ => short(at, now),
    }
}

/// Why the picker's text doesn't give a time, in the line under its field.
pub const NOT_READ: &str = "Not a day or time ilo can read";
pub const PAST: &str = "That time has passed";

/// What the picker's `text` names at `now`: a day, a time or both, such as "Friday 17:30",
/// "24 Nov 09:00", "tomorrow", "5pm" or "17時30分". A day alone comes back in the morning, and a
/// time alone at its next turn of the clock. It must be after `now`.
pub fn read(text: &str, now: NaiveDateTime) -> Result<NaiveDateTime, &'static str> {
    let text = halfwidth(text).to_lowercase();
    let text = text
        .replace("今日", " today ")
        .replace("明日", " tomorrow ")
        .replace('時', ":")
        .replace('分', " ")
        .replace(',', " ");
    let mut day: Option<Day> = None;
    let mut time: Option<NaiveTime> = None;
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|word| *word != "at")
        .collect();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        let next = words.get(index + 1).copied();
        // "5 pm", the meridiem as a word of its own.
        if let Some(meridiem @ ("am" | "pm")) = next
            && let Some(at) = clock(&format!("{word}{meridiem}"))
            && time.is_none()
        {
            time = Some(at);
            index += 2;
            continue;
        }
        // "24 Nov" and "Nov 24", with a year after them or not, and after a weekday as the
        // picker's own text has it, "Wed 11 Nov 08:00".
        if matches!(day, None | Some(Day::Weekday(_)))
            && let Some((date, used)) = date(&words[index..], now.date())
        {
            day = Some(Day::Date(date));
            index += used;
            continue;
        }
        if day.is_none()
            && let Some(named) = named_day(word)
        {
            day = Some(named);
            index += 1;
            continue;
        }
        if time.is_none()
            && let Some(at) = clock(word)
        {
            time = Some(at);
            index += 1;
            continue;
        }
        return Err(NOT_READ);
    }
    let at = match (day, time) {
        (None, None) => return Err(NOT_READ),
        (None, Some(time)) => {
            let today = now.date().and_time(time);
            match today > now {
                true => today,
                false => (now.date() + Days::new(1)).and_time(time),
            }
        }
        (Some(day), time) => {
            let time = time.unwrap_or(hm(MORNING));
            match day {
                Day::Date(date) => date.and_time(time),
                Day::Today => now.date().and_time(time),
                Day::Tomorrow => (now.date() + Days::new(1)).and_time(time),
                Day::Weekday(weekday) => {
                    let ahead = (7 + weekday.num_days_from_monday()
                        - now.weekday().num_days_from_monday())
                        % 7;
                    let at = (now.date() + Days::new(ahead.into())).and_time(time);
                    match at > now {
                        true => at,
                        false => at + Days::new(7),
                    }
                }
            }
        }
    };
    match at > now {
        true => Ok(at),
        false => Err(PAST),
    }
}

#[derive(Clone, Copy)]
enum Day {
    Today,
    Tomorrow,
    Weekday(Weekday),
    Date(NaiveDate),
}

fn named_day(word: &str) -> Option<Day> {
    match word {
        "today" | "tonight" => return Some(Day::Today),
        "tomorrow" | "tmrw" | "tmr" => return Some(Day::Tomorrow),
        _ => {}
    }
    let weekdays = [
        ("monday", Weekday::Mon),
        ("tuesday", Weekday::Tue),
        ("wednesday", Weekday::Wed),
        ("thursday", Weekday::Thu),
        ("friday", Weekday::Fri),
        ("saturday", Weekday::Sat),
        ("sunday", Weekday::Sun),
    ];
    // "fri", "frid" and "friday" all name Friday; "t" and "s" name two days, so three letters.
    (word.len() >= 3)
        .then(|| weekdays.iter().find(|(name, _)| name.starts_with(word)))
        .flatten()
        .map(|&(_, weekday)| Day::Weekday(weekday))
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches('.');
    (word.len() >= 3)
        .then(|| MONTHS.iter().position(|name| name.starts_with(word)))
        .flatten()
        .map(|index| index as u32 + 1)
}

/// A date at the start of `words` and how many words it took: "24 nov", "nov 24", either with a
/// year after it, "2026-11-24", or "11月24日". Without a year, the next one to come.
fn date(words: &[&str], today: NaiveDate) -> Option<(NaiveDate, usize)> {
    let first = *words.first()?;
    if let Ok(date) = NaiveDate::parse_from_str(first, "%Y-%m-%d") {
        return Some((date, 1));
    }
    if let Some((month, rest)) = first.split_once('月')
        && let Some(day) = rest.strip_suffix('日')
        && let (Ok(month), Ok(day)) = (month.parse(), day.parse())
    {
        return Some((next_date(month, day, None, today)?, 1));
    }
    let second = *words.get(1)?;
    let (month, day) = match (month(first), month(second)) {
        (None, Some(month)) => (month, first.parse().ok()?),
        (Some(month), None) => (month, second.trim_end_matches(',').parse().ok()?),
        _ => return None,
    };
    let year = words
        .get(2)
        .filter(|word| word.len() == 4)
        .and_then(|word| word.parse().ok());
    let date = next_date(month, day, year, today)?;
    Some((date, if year.is_some() { 3 } else { 2 }))
}

fn next_date(month: u32, day: u32, year: Option<i32>, today: NaiveDate) -> Option<NaiveDate> {
    if let Some(year) = year {
        return NaiveDate::from_ymd_opt(year, month, day);
    }
    let this_year = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    match this_year >= today {
        true => Some(this_year),
        false => NaiveDate::from_ymd_opt(today.year() + 1, month, day),
    }
}

/// A time of day: "17:30", "17.30", "9", "9am", "5:30pm", "noon", or "17:" as "17時" leaves it.
fn clock(word: &str) -> Option<NaiveTime> {
    match word {
        "noon" | "midday" => return NaiveTime::from_hms_opt(12, 0, 0),
        "midnight" => return NaiveTime::from_hms_opt(0, 0, 0),
        _ => {}
    }
    let (digits, meridiem) = match word.strip_suffix("am") {
        Some(digits) => (digits, Some(false)),
        None => match word.strip_suffix("pm") {
            Some(digits) => (digits, Some(true)),
            None => (word, None),
        },
    };
    let (hour, minute) = match digits.split_once([':', '.']) {
        Some((hour, "")) => (hour, "0"),
        Some((hour, minute)) if minute.len() == 2 => (hour, minute),
        Some(_) => return None,
        None => (digits, "0"),
    };
    if hour.is_empty() || hour.len() > 2 || !hour.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (mut hour, minute): (u32, u32) = (hour.parse().ok()?, minute.parse().ok()?);
    match meridiem {
        Some(_) if !(1..=12).contains(&hour) => return None,
        Some(pm) => hour = hour % 12 + if pm { 12 } else { 0 },
        None => {}
    }
    NaiveTime::from_hms_opt(hour, minute, 0)
}

/// Full-width letters, digits and signs, as a Japanese input method types them, as ASCII.
fn halfwidth(text: &str) -> String {
    text.chars()
        .map(|char| match char {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(char as u32 - 0xFEE0).unwrap_or(char),
            char => char,
        })
        .collect()
}

#[cfg(test)]
mod tests;
