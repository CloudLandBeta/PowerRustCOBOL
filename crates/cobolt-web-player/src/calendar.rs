// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The calendar of a date field** (2.0.432): what a DateTimePicker that edits a date alone offers on a Web or Mobile page.
//! The user types the date into the field or presses the calendar button beside it and picks a day; the calendar jumps to
//! any month and year, and marks the Catholic holy days that are public holidays for the whole country of the page's locale.
//!
//! Nothing here knows a browser. The month grid, the holidays, the texts in the six languages and the reading of a typed
//! date are plain arithmetic, checked by native tests; `view::datefield` draws them and `view::interact` answers the presses.
//! `Value` stays what it always was: ISO `YYYY-MM-DD`, whatever is shown or typed.

use cobolt_forms::fieldtext::{day_of_week, days_in_month, step_date};

/// A date: year, month (1-12), day (1-31).
pub type Ymd = (i32, u32, u32);

/// `YYYY-MM-DD`, the form `Value` holds.
pub fn iso(d: Ymd) -> String {
    format!("{:04}-{:02}-{:02}", d.0, d.1, d.2)
}

// ── the month on show ───────────────────────────────────────────────────────

/// The 42 days a month's grid shows: six weeks from the Sunday on or before the 1st, so the days of the month before and
/// of the month after that share its weeks are there too (and can be picked).
pub fn month_cells(year: i32, month: u32) -> Vec<Ymd> {
    let month = month.clamp(1, 12);
    let lead = day_of_week(year, month, 1) as i32;
    let first = step_date((year, month, 1), -lead);
    (0..42).map(|k| step_date(first, k)).collect()
}

/// The month before (`-1`) or after (`+1`) a month.
pub fn step_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let at = year * 12 + month.clamp(1, 12) as i32 - 1 + delta;
    (at.div_euclid(12), at.rem_euclid(12) as u32 + 1)
}

/// The years the calendar's year list offers: from the earliest date the field takes to the latest, or a century back and
/// ten years on from today when it has no such bound; the year on show is always among them.
pub fn year_range(shown: i32, today: i32, bounds: (Option<Ymd>, Option<Ymd>)) -> (i32, i32) {
    let lo = bounds.0.map(|d| d.0).unwrap_or(today - 100).min(shown);
    let hi = bounds.1.map(|d| d.0).unwrap_or(today + 10).max(shown);
    (lo, hi.max(lo))
}

// ── the holy days ───────────────────────────────────────────────────────────

/// Easter Sunday of a year of the Gregorian calendar, as `(month, day)` (the anonymous algorithm of Meeus, Jones and Butcher).
pub fn easter(year: i32) -> (u32, u32) {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let n = h + l - 7 * m + 114;
    ((n / 31) as u32, (n % 31 + 1) as u32)
}

/// A Catholic holy day the calendar can mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holiday {
    Epiphany,
    GoodFriday,
    Easter,
    EasterMonday,
    Ascension,
    WhitMonday,
    CorpusChristi,
    Assumption,
    Aparecida,
    AllSaints,
    AllSouls,
    ImmaculateConception,
    Christmas,
}

impl Holiday {
    /// Its date in `year`: a fixed day, or so many days from Easter Sunday.
    pub fn date(self, year: i32) -> Ymd {
        let fixed = |m, d| (year, m, d);
        let from_easter = |days: i32| {
            let (m, d) = easter(year);
            step_date((year, m, d), days)
        };
        match self {
            Holiday::Epiphany => fixed(1, 6),
            Holiday::GoodFriday => from_easter(-2),
            Holiday::Easter => from_easter(0),
            Holiday::EasterMonday => from_easter(1),
            Holiday::Ascension => from_easter(39),
            Holiday::WhitMonday => from_easter(50),
            Holiday::CorpusChristi => from_easter(60),
            Holiday::Assumption => fixed(8, 15),
            Holiday::Aparecida => fixed(10, 12),
            Holiday::AllSaints => fixed(11, 1),
            Holiday::AllSouls => fixed(11, 2),
            Holiday::ImmaculateConception => fixed(12, 8),
            Holiday::Christmas => fixed(12, 25),
        }
    }

    /// Its name in the language of `locale` (the six of the IDE; English otherwise).
    pub fn name(self, locale: &str) -> &'static str {
        // en, pt, es, fr, ja, zh
        let names: [&str; 6] = match self {
            Holiday::Epiphany => ["Epiphany", "Dia de Reis", "Epifanía del Señor", "Épiphanie", "公現祭", "主显节"],
            Holiday::GoodFriday => ["Good Friday", "Sexta-feira Santa", "Viernes Santo", "Vendredi saint", "聖金曜日", "耶稣受难日"],
            Holiday::Easter => ["Easter Sunday", "Páscoa", "Domingo de Pascua", "Pâques", "復活祭", "复活节"],
            Holiday::EasterMonday => ["Easter Monday", "Segunda-feira de Páscoa", "Lunes de Pascua", "Lundi de Pâques", "復活祭の月曜日", "复活节星期一"],
            Holiday::Ascension => ["Ascension Day", "Ascensão do Senhor", "Ascensión del Señor", "Ascension", "昇天祭", "耶稣升天节"],
            Holiday::WhitMonday => ["Whit Monday", "Segunda-feira de Pentecostes", "Lunes de Pentecostés", "Lundi de Pentecôte", "聖霊降臨祭の月曜日", "圣灵降临节星期一"],
            Holiday::CorpusChristi => ["Corpus Christi", "Corpus Christi", "Corpus Christi", "Fête-Dieu", "聖体祭", "基督圣体节"],
            Holiday::Assumption => ["Assumption of Mary", "Assunção de Nossa Senhora", "Asunción de la Virgen", "Assomption", "聖母被昇天祭", "圣母升天节"],
            Holiday::Aparecida => ["Our Lady of Aparecida", "Nossa Senhora Aparecida", "Nuestra Señora de Aparecida", "Notre-Dame d’Aparecida", "アパレシーダの聖母の日", "阿帕雷西达圣母节"],
            Holiday::AllSaints => ["All Saints’ Day", "Dia de Todos os Santos", "Todos los Santos", "Toussaint", "諸聖人の日", "诸圣节"],
            Holiday::AllSouls => ["All Souls’ Day", "Finados", "Día de los Fieles Difuntos", "Jour des défunts", "死者の日", "追思节"],
            Holiday::ImmaculateConception => ["Immaculate Conception", "Imaculada Conceição", "Inmaculada Concepción", "Immaculée Conception", "無原罪の聖マリアの祭日", "圣母无染原罪节"],
            Holiday::Christmas => ["Christmas Day", "Natal", "Navidad", "Noël", "クリスマス", "圣诞节"],
        };
        names[lang_index(locale)]
    }
}

/// The country a locale speaks of, as two capital letters: its region (`pt-BR` is `BR`, `fr_CA` is `CA`), else the country
/// the IDE's own translation of that language is written for (`pt` is Brazil, `es` Spain, `fr` France, `ja` Japan, `zh`
/// China, anything else the United States).
pub fn country(locale: &str) -> String {
    let mut parts = locale.trim().split(['-', '_']);
    let lang = parts.next().unwrap_or("").to_ascii_lowercase();
    if let Some(region) = parts.find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic())) {
        return region.to_ascii_uppercase();
    }
    match lang.as_str() {
        "pt" => "BR",
        "es" => "ES",
        "fr" => "FR",
        "ja" => "JP",
        "zh" => "CN",
        _ => "US",
    }
    .to_owned()
}

/// The Catholic holy days that everybody in a country keeps: the ones that are public holidays in the whole of it (never one
/// kept by a region, a city or by choice), and Easter Sunday. A country the table does not know keeps the two that are kept
/// wherever the calendar is Christian; one where no Catholic day is a holiday keeps none.
pub fn observed(country: &str) -> &'static [Holiday] {
    use Holiday::*;
    match country {
        "BR" => &[GoodFriday, Easter, Aparecida, AllSouls, Christmas],
        "PT" => &[GoodFriday, Easter, CorpusChristi, Assumption, AllSaints, ImmaculateConception, Christmas],
        "ES" => &[Epiphany, GoodFriday, Easter, Assumption, AllSaints, ImmaculateConception, Christmas],
        "FR" => &[Easter, EasterMonday, Ascension, WhitMonday, Assumption, AllSaints, Christmas],
        "JP" | "CN" => &[],
        _ => &[Easter, Christmas],
    }
}

/// The holy day `date` is in `country`, if it is one.
pub fn holiday_on(country: &str, date: Ymd) -> Option<Holiday> {
    observed(country).iter().copied().find(|h| h.date(date.0) == date)
}

// ── the texts ───────────────────────────────────────────────────────────────

/// What the calendar says, in one language.
pub struct CalendarTexts {
    /// A BCP 47 tag; its first subtag is the language.
    pub lang: &'static str,
    pub months: [&'static str; 12],
    /// Sunday first.
    pub weekdays: [&'static str; 7],
    pub open: &'static str,
    pub previous: &'static str,
    pub next: &'static str,
    pub month: &'static str,
    pub year: &'static str,
    pub today: &'static str,
    pub clear: &'static str,
    pub close: &'static str,
}

pub const CALENDAR_TEXTS: [CalendarTexts; 6] = [
    CalendarTexts {
        lang: "en",
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        weekdays: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        open: "Open the calendar",
        previous: "Previous month",
        next: "Next month",
        month: "Month",
        year: "Year",
        today: "Today",
        clear: "Clear",
        close: "Close",
    },
    CalendarTexts {
        lang: "pt-BR",
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        weekdays: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
        open: "Abrir o calendário",
        previous: "Mês anterior",
        next: "Próximo mês",
        month: "Mês",
        year: "Ano",
        today: "Hoje",
        clear: "Limpar",
        close: "Fechar",
    },
    CalendarTexts {
        lang: "es",
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        weekdays: ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
        open: "Abrir el calendario",
        previous: "Mes anterior",
        next: "Mes siguiente",
        month: "Mes",
        year: "Año",
        today: "Hoy",
        clear: "Borrar",
        close: "Cerrar",
    },
    CalendarTexts {
        lang: "fr",
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        weekdays: ["dim", "lun", "mar", "mer", "jeu", "ven", "sam"],
        open: "Ouvrir le calendrier",
        previous: "Mois précédent",
        next: "Mois suivant",
        month: "Mois",
        year: "Année",
        today: "Aujourd’hui",
        clear: "Effacer",
        close: "Fermer",
    },
    CalendarTexts {
        lang: "ja",
        months: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        weekdays: ["日", "月", "火", "水", "木", "金", "土"],
        open: "カレンダーを開く",
        previous: "前の月",
        next: "次の月",
        month: "月",
        year: "年",
        today: "今日",
        clear: "クリア",
        close: "閉じる",
    },
    CalendarTexts {
        lang: "zh",
        months: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        weekdays: ["日", "一", "二", "三", "四", "五", "六"],
        open: "打开日历",
        previous: "上个月",
        next: "下个月",
        month: "月份",
        year: "年份",
        today: "今天",
        clear: "清除",
        close: "关闭",
    },
];

/// Which of the six languages a locale is in (its first subtag); English when it is none of them.
fn lang_index(locale: &str) -> usize {
    let first = locale.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    CALENDAR_TEXTS.iter().position(|t| t.lang.split('-').next() == Some(first.as_str())).unwrap_or(0)
}

/// The calendar's texts in the language of `locale`.
pub fn texts(locale: &str) -> &'static CalendarTexts {
    &CALENDAR_TEXTS[lang_index(locale)]
}

/// A day as a screen reader says it, with its holy day when it is one: `25 dezembro 2026, Natal`.
pub fn day_label(locale: &str, date: Ymd, holiday: Option<Holiday>) -> String {
    let month = texts(locale).months[(date.1.clamp(1, 12) - 1) as usize];
    match holiday {
        Some(h) => format!("{} {month} {}, {}", date.2, date.0, h.name(locale)),
        None => format!("{} {month} {}", date.2, date.0),
    }
}

// ── a date that is typed ────────────────────────────────────────────────────

/// How a date is typed into a field: which of day, month and year, in which order, and what is written between them. It
/// follows what the field shows: `Short` and `Long` are typed as `yyyy-MM-dd` (what `Short` shows, and what `Value` holds),
/// a `Custom` format in the order and with the separator of its own pattern, each part as a number
/// (`d MMMM yyyy` is typed `dd MM yyyy`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    /// `d`, `M` and `y`, in the order they are typed. A part the pattern leaves out is not typed (`MM/yyyy`: the day is the 1st).
    pub parts: Vec<char>,
    pub separator: char,
}

impl Typed {
    /// From a DateTimePicker's `Format` and `CustomFormat`.
    pub fn of(format: &str, custom: &str) -> Typed {
        let iso = Typed { parts: vec!['y', 'M', 'd'], separator: '-' };
        if !format.trim().eq_ignore_ascii_case("custom") {
            return iso;
        }
        let chars: Vec<char> = custom.trim().chars().collect();
        let mut parts = Vec::new();
        let mut separator = None;
        let mut k = 0;
        while k < chars.len() {
            let c = chars[k];
            let mut n = 1;
            while k + n < chars.len() && chars[k + n] == c {
                n += 1;
            }
            match c {
                // `ddd` and `dddd` are the weekday's name: shown, never typed.
                'd' if n >= 3 => {}
                'd' | 'M' | 'y' if !parts.contains(&c) => parts.push(c),
                // What stands between the first two parts is what is typed between all of them.
                _ if parts.len() == 1 && separator.is_none() && !c.is_alphanumeric() => separator = Some(c),
                _ => {}
            }
            k += n;
        }
        // A pattern that names no year cannot say which day it is: the ISO form is typed.
        if !parts.contains(&'y') {
            return iso;
        }
        Typed { parts, separator: separator.unwrap_or('/') }
    }

    /// The pattern, as the empty field shows it: `dd/MM/yyyy`, `yyyy-MM-dd`.
    pub fn pattern(&self) -> String {
        let sep = self.separator.to_string();
        self.parts
            .iter()
            .map(|p| match p {
                'd' => "dd",
                'M' => "MM",
                _ => "yyyy",
            })
            .collect::<Vec<_>>()
            .join(&sep)
    }

    /// `date` as it is typed: `11/11/2026`.
    pub fn show(&self, date: Ymd) -> String {
        let sep = self.separator.to_string();
        self.parts
            .iter()
            .map(|p| match p {
                'd' => format!("{:02}", date.2),
                'M' => format!("{:02}", date.1),
                _ => format!("{:04}", date.0),
            })
            .collect::<Vec<_>>()
            .join(&sep)
    }

    /// The date `text` says, or `None` when it says none. The parts are read in the field's order, with anything that is
    /// not a digit between them (`11/11/2026`, `11.11.2026`, `11 11 2026`), or as digits alone, two for the day, two for the
    /// month and four or two for the year (`11112026`, `111126`). A year of two digits is of this century below 50 and of
    /// the last one from 50 on. A day the month does not have is no date.
    pub fn read(&self, text: &str) -> Option<Ymd> {
        let groups: Vec<&str> = text.split(|c: char| !c.is_ascii_digit()).filter(|g| !g.is_empty()).collect();
        let numbers: Vec<(u32, usize)> = if groups.len() == self.parts.len() {
            groups.iter().map(|g| Some((g.parse().ok()?, g.len()))).collect::<Option<_>>()?
        } else if groups.len() == 1 && self.parts.len() > 1 {
            // Digits alone: cut by each part's width, with a year of four digits or of two.
            let run = groups[0];
            let short = self.parts.len() * 2;
            let year_width = match run.len() {
                n if n == short + 2 => 4,
                n if n == short => 2,
                _ => return None,
            };
            let mut at = 0;
            let mut out = Vec::new();
            for p in &self.parts {
                let w = if *p == 'y' { year_width } else { 2 };
                out.push((run[at..at + w].parse().ok()?, w));
                at += w;
            }
            out
        } else {
            return None;
        };
        let (mut year, mut month, mut day) = (None, 1, 1);
        for (p, (n, width)) in self.parts.iter().zip(numbers) {
            match p {
                'd' => day = n,
                'M' => month = n,
                _ => {
                    year = Some(match width {
                        1 | 2 if n < 50 => 2000 + n as i32,
                        1 | 2 => 1900 + n as i32,
                        _ => n as i32,
                    })
                }
            }
        }
        let year = year.filter(|y| (1..=9999).contains(y))?;
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)).then_some((year, month, day))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easter_sunday_is_the_churchs_date_in_every_year_checked() {
        // (year, month, day): the earliest and the latest Easter there can be, and the years around today.
        let known = [(1818, 3, 22), (1943, 4, 25), (2000, 4, 23), (2008, 3, 23), (2019, 4, 21), (2024, 3, 31), (2025, 4, 20), (2026, 4, 5), (2027, 3, 28), (2028, 4, 16), (2038, 4, 25), (2285, 3, 22)];
        for (y, m, d) in known {
            assert_eq!(easter(y), (m, d), "Easter of {y}");
            assert_eq!(day_of_week(y, m, d), 0, "Easter of {y} is a Sunday");
        }
        println!("{} years: Easter Sunday as the Church keeps it, each one a Sunday", known.len());
    }

    #[test]
    fn each_country_marks_the_holy_days_everybody_keeps_there_and_no_other() {
        let on = |country: &str, y: i32, m: u32| -> Vec<(u32, Holiday)> {
            (1..=days_in_month(y, m)).filter_map(|d| holiday_on(country, (y, m, d)).map(|h| (d, h))).collect()
        };
        // Brazil, 2026: Good Friday is 3 April and Easter 5 April; 12 October, 2 November, 25 December.
        assert_eq!(on("BR", 2026, 4), [(3, Holiday::GoodFriday), (5, Holiday::Easter)]);
        assert_eq!(on("BR", 2026, 10), [(12, Holiday::Aparecida)]);
        assert_eq!(on("BR", 2026, 11), [(2, Holiday::AllSouls)]);
        assert_eq!(on("BR", 2026, 12), [(25, Holiday::Christmas)]);
        // Corpus Christi (4 June 2026) is kept by choice in Brazil, by law in Portugal.
        assert_eq!(on("BR", 2026, 6), []);
        assert_eq!(on("PT", 2026, 6), [(4, Holiday::CorpusChristi)]);
        // Spain keeps the Epiphany; France the Monday after Easter, the Ascension (14 May 2026) and Whit Monday (25 May).
        assert_eq!(on("ES", 2026, 1), [(6, Holiday::Epiphany)]);
        assert_eq!(on("FR", 2026, 4), [(5, Holiday::Easter), (6, Holiday::EasterMonday)]);
        assert_eq!(on("FR", 2026, 5), [(14, Holiday::Ascension), (25, Holiday::WhitMonday)]);
        assert_eq!(on("ES", 2026, 12), [(8, Holiday::ImmaculateConception), (25, Holiday::Christmas)]);
        // A country the table does not know keeps Easter and Christmas; Japan and China keep none.
        assert_eq!(on("US", 2026, 12), [(25, Holiday::Christmas)]);
        assert_eq!(on("MX", 2026, 4), [(5, Holiday::Easter)]);
        assert!(on("JP", 2026, 12).is_empty() && on("CN", 2026, 4).is_empty());
        // The country is the locale's region, else the home of the IDE's own translation of the language.
        for (locale, want) in [("pt-BR", "BR"), ("pt_PT", "PT"), ("pt", "BR"), ("es", "ES"), ("es-MX", "MX"), ("fr-CA", "CA"), ("fr", "FR"), ("ja", "JP"), ("zh-Hans-CN", "CN"), ("zh", "CN"), ("en", "US"), ("en-GB", "GB"), ("", "US")] {
            assert_eq!(country(locale), want, "{locale:?}");
        }
    }

    #[test]
    fn every_text_of_the_calendar_and_every_holy_day_has_its_six_languages() {
        let langs: Vec<&str> = CALENDAR_TEXTS.iter().map(|t| t.lang).collect();
        assert_eq!(langs, ["en", "pt-BR", "es", "fr", "ja", "zh"]);
        let mut blank = Vec::new();
        for t in &CALENDAR_TEXTS {
            let all = t.months.iter().chain(&t.weekdays).chain([&t.open, &t.previous, &t.next, &t.month, &t.year, &t.today, &t.clear, &t.close]);
            blank.extend(all.filter(|s| s.trim().is_empty()).map(|_| t.lang));
        }
        assert!(blank.is_empty(), "blank calendar texts: {blank:?}");
        use Holiday::*;
        let days = [Epiphany, GoodFriday, Easter, EasterMonday, Ascension, WhitMonday, CorpusChristi, Assumption, Aparecida, AllSaints, AllSouls, ImmaculateConception, Christmas];
        for h in days {
            for t in &CALENDAR_TEXTS {
                assert!(!h.name(t.lang).trim().is_empty(), "{h:?} in {}", t.lang);
            }
        }
        assert_eq!(Holiday::Christmas.name("pt-BR"), "Natal");
        assert_eq!(Holiday::AllSouls.name("pt"), "Finados");
        assert_eq!(Holiday::GoodFriday.name("es-MX"), "Viernes Santo");
        assert_eq!(Holiday::Christmas.name("de"), "Christmas Day", "a language that is none of the six reads English");
        assert_eq!(texts("fr-CA").today, "Aujourd’hui");
        assert_eq!(day_label("pt-BR", (2026, 12, 25), Some(Holiday::Christmas)), "25 dezembro 2026, Natal");
        assert_eq!(day_label("en", (2026, 11, 11), None), "11 November 2026");
        println!("{} texts and {} holy days, each in {} languages", 12 + 7 + 8, days.len(), CALENDAR_TEXTS.len());
    }

    #[test]
    fn a_month_is_shown_as_six_weeks_from_a_sunday_and_the_lists_reach_every_year_asked_for() {
        // November 2026 begins on a Sunday: the grid begins on the 1st and runs into December.
        let nov = month_cells(2026, 11);
        assert_eq!((nov.len(), nov[0], nov[29], nov[30], nov[41]), (42, (2026, 11, 1), (2026, 11, 30), (2026, 12, 1), (2026, 12, 12)));
        // January 2027 begins on a Friday: five days of December 2026 lead it.
        let jan = month_cells(2027, 1);
        assert_eq!((jan[0], jan[4], jan[5]), ((2026, 12, 27), (2026, 12, 31), (2027, 1, 1)));
        assert!(jan.iter().step_by(7).all(|d| day_of_week(d.0, d.1, d.2) == 0), "every row begins on a Sunday");
        // February of a leap year.
        assert!(month_cells(2028, 2).contains(&(2028, 2, 29)) && !month_cells(2027, 2).contains(&(2027, 2, 29)));
        assert_eq!((step_month(2026, 12, 1), step_month(2026, 1, -1), step_month(2026, 6, 1)), ((2027, 1), (2025, 12), (2026, 7)));
        // The years offered: a century back and ten on, the field's own bounds when it has them, the year on show always.
        assert_eq!(year_range(2026, 2026, (None, None)), (1926, 2036));
        assert_eq!(year_range(1850, 2026, (None, None)), (1850, 2036));
        assert_eq!(year_range(2026, 2026, (Some((2020, 1, 1)), Some((2030, 12, 31)))), (2020, 2030));
        assert_eq!(year_range(2040, 2026, (Some((2020, 1, 1)), Some((2030, 12, 31)))), (2020, 2040));
    }

    #[test]
    fn a_typed_date_is_read_in_the_order_the_field_shows_it_with_or_without_separators() {
        let br = Typed::of("Custom", "dd/MM/yyyy");
        assert_eq!((br.pattern(), br.show((2026, 11, 3))), ("dd/MM/yyyy".to_owned(), "03/11/2026".to_owned()));
        for text in ["03/11/2026", "3/11/2026", "03.11.2026", "03-11-2026", "03 11 2026", " 03/11/2026 ", "03112026", "031126", "3/11/26"] {
            assert_eq!(br.read(text), Some((2026, 11, 3)), "{text:?}");
        }
        assert_eq!(br.read("03/11/85"), Some((1985, 11, 3)), "a year of two digits from 50 on is of the last century");
        // No date: a day the month does not have, a month there is not, too few or too many digits, words.
        for text in ["", "31/02/2026", "29/02/2027", "03/13/2026", "00/11/2026", "0311202", "031120261", "03/11", "amanhã", "03/11/0000"] {
            assert_eq!(br.read(text), None, "{text:?}");
        }
        assert_eq!(br.read("29/02/2028"), Some((2028, 2, 29)), "the 29th of February of a leap year");
        // `Short` and `Long` are typed as ISO, what `Short` shows and `Value` holds.
        for format in ["Short", "Long", "", "short"] {
            let iso_typed = Typed::of(format, "dd/MM/yyyy");
            assert_eq!((iso_typed.pattern(), iso_typed.read("2026-11-03"), iso_typed.read("20261103")), ("yyyy-MM-dd".to_owned(), Some((2026, 11, 3)), Some((2026, 11, 3))), "{format:?}");
        }
        // The order and the separator are the pattern's own; a month's or a weekday's name is typed as its number or not at all.
        let us = Typed::of("Custom", "MM-dd-yyyy");
        assert_eq!((us.pattern(), us.read("11-03-2026"), us.show((2026, 11, 3))), ("MM-dd-yyyy".to_owned(), Some((2026, 11, 3)), "11-03-2026".to_owned()));
        let long = Typed::of("Custom", "dddd, d MMMM yyyy");
        assert_eq!((long.pattern(), long.read("3 11 2026")), ("dd MM yyyy".to_owned(), Some((2026, 11, 3))));
        let dotted = Typed::of("custom", "d.M.yy");
        assert_eq!((dotted.pattern(), dotted.read("3.11.26")), ("dd.MM.yyyy".to_owned(), Some((2026, 11, 3))));
        // A month and a year alone: the day is the 1st.
        let month = Typed::of("Custom", "MM/yyyy");
        assert_eq!((month.pattern(), month.read("11/2026"), month.read("112026")), ("MM/yyyy".to_owned(), Some((2026, 11, 1)), Some((2026, 11, 1))));
        // A pattern with no year, or none at all, is typed as ISO.
        assert_eq!(Typed::of("Custom", "dd/MM").pattern(), "yyyy-MM-dd");
        assert_eq!(Typed::of("Custom", "").pattern(), "yyyy-MM-dd");
        assert_eq!(iso((2026, 1, 5)), "2026-01-05");
    }
}
