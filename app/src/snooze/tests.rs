use chrono::{NaiveDate, NaiveDateTime};

use super::*;

/// Tuesday 10 November 2026, 14:25, the mocks' clock.
fn tuesday() -> NaiveDateTime {
    at(2026, 11, 10, 14, 25)
}

fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(hour, minute, 0)
        .unwrap()
}

#[test]
fn the_short_list_means_three_hours_on_six_tonight_eight_tomorrow_and_eight_on_monday() {
    let now = tuesday();
    let times: Vec<_> = Choice::ALL
        .iter()
        .map(|choice| choice.at(now).map(|at| short(at, now)))
        .collect();
    assert_eq!(
        times,
        [
            Some("17:25".into()),
            Some("18:00".into()),
            Some("Wed 08:00".into()),
            Some("Mon 08:00".into())
        ]
    );
}

#[test]
fn this_evening_goes_from_the_list_once_its_evening() {
    assert_eq!(Choice::ThisEvening.at(at(2026, 11, 10, 17, 45)), None);
    assert_eq!(Choice::ThisEvening.at(at(2026, 11, 10, 20, 0)), None);
}

#[test]
fn next_week_on_a_monday_is_the_monday_after() {
    let monday = at(2026, 11, 9, 9, 0);
    assert_eq!(Choice::NextWeek.at(monday), Some(at(2026, 11, 16, 8, 0)));
}

#[test]
fn later_today_late_at_night_says_its_day() {
    let late = at(2026, 11, 10, 22, 10);
    let later = Choice::LaterToday.at(late).unwrap();
    assert_eq!(short(later, late), "Wed 01:10");
}

#[test]
fn the_picker_reads_days_and_times_as_people_type_them() {
    let now = tuesday();
    let read = |text: &str| read(text, now).map(|at| full(at, now));
    assert_eq!(read("Friday 17:30"), Ok("Fri 13 Nov 17:30".into()));
    assert_eq!(read("fri 5pm"), Ok("Fri 13 Nov 17:00".into()));
    assert_eq!(read("fri 5 pm"), Ok("Fri 13 Nov 17:00".into()));
    assert_eq!(read("24 Nov 09:00"), Ok("Tue 24 Nov 09:00".into()));
    assert_eq!(read("Nov 24 at 9am"), Ok("Tue 24 Nov 09:00".into()));
    assert_eq!(read("2026-12-01 18:15"), Ok("Tue 1 Dec 18:15".into()));
    assert_eq!(read("tomorrow"), Ok("Wed 11 Nov 08:00".into()));
    assert_eq!(read("Wed 11 Nov 08:00"), Ok("Wed 11 Nov 08:00".into()));
}

#[test]
fn a_day_alone_comes_back_in_the_morning_and_a_time_alone_at_its_next_turn() {
    let now = tuesday();
    assert_eq!(read("thursday", now), Ok(at(2026, 11, 12, 8, 0)));
    assert_eq!(read("16:00", now), Ok(at(2026, 11, 10, 16, 0)));
    assert_eq!(read("9", now), Ok(at(2026, 11, 11, 9, 0)));
}

#[test]
fn todays_weekday_is_today_while_its_time_is_ahead_else_next_week() {
    let now = tuesday();
    assert_eq!(read("tuesday 18:00", now), Ok(at(2026, 11, 10, 18, 0)));
    assert_eq!(read("tuesday 09:00", now), Ok(at(2026, 11, 17, 9, 0)));
}

#[test]
fn a_date_gone_this_year_is_next_years_and_says_so() {
    let now = tuesday();
    let read = read("3 Jan", now).unwrap();
    assert_eq!(full(read, now), "Sun 3 Jan 2027 08:00");
}

#[test]
fn what_doesnt_read_as_a_time_ahead_says_why() {
    let now = tuesday();
    assert_eq!(read("", now), Err(NOT_READ));
    assert_eq!(read("whenever", now), Err(NOT_READ));
    assert_eq!(read("25:00", now), Err(NOT_READ));
    assert_eq!(read("31 Feb", now), Err(NOT_READ));
    assert_eq!(read("today 09:00", now), Err(PAST));
    assert_eq!(read("2026-11-01 10:00", now), Err(PAST));
}

#[test]
fn a_japanese_input_methods_full_width_text_reads_too() {
    let now = tuesday();
    assert_eq!(read("１７：３０", now), Ok(at(2026, 11, 10, 17, 30)));
    assert_eq!(read("明日 9時30分", now), Ok(at(2026, 11, 11, 9, 30)));
    assert_eq!(read("11月24日 17時", now), Ok(at(2026, 11, 24, 17, 0)));
}

#[test]
fn the_toast_says_the_time_in_a_sentence() {
    let now = tuesday();
    assert_eq!(words(at(2026, 11, 10, 17, 25), now), "today 17:25");
    assert_eq!(words(at(2026, 11, 11, 8, 0), now), "tomorrow 08:00");
    assert_eq!(words(at(2026, 11, 16, 8, 0), now), "Mon 08:00");
}

#[test]
fn a_time_goes_to_the_core_and_back_unchanged() {
    let time = at(2026, 11, 13, 17, 30);
    assert_eq!(local(millis(time)), Some(time));
}
