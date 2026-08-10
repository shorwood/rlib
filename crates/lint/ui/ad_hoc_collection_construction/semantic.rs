#![warn(ad_hoc_collection_construction)]
#![allow(
    dead_code,
    misordered_module_declarations,
    missing_section_dividers,
    non_adjacent_struct_impls
)]

struct Entry(u64);

struct Report {
    entries: Vec<Entry>,
}

impl Report {
    fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut report = Self {
            entries: Vec::new(),
        };
        report.entries.extend(entries);
        report
    }

    fn add_all_entries(&mut self, entries: impl IntoIterator<Item = Entry>) {
        self.entries.extend(entries);
    }
}

struct FreeReport {
    entries: Vec<Entry>,
}

fn free_report_from_entries(entries: impl IntoIterator<Item = Entry>) -> FreeReport {
    let mut report = FreeReport {
        entries: Vec::new(),
    };
    for entry in entries {
        report.entries.push(entry);
    }
    report
}

fn add_all_free_report_entries(
    report: &mut FreeReport,
    entries: impl IntoIterator<Item = Entry>,
) {
    report.entries.extend(entries);
}

struct FilteredReport {
    entries: Vec<Entry>,
}

impl FilteredReport {
    fn from_filtered_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut report = Self {
            entries: Vec::new(),
        };
        report.entries.extend(entries.into_iter().filter(|entry| entry.0 > 0));
        report
    }
}

fn main() {}
