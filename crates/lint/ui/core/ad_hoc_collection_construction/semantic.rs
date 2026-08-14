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

struct CollectedReport {
    entries: Vec<Entry>,
}

impl CollectedReport {
    // False-negative boundary: direct `collect` initialization is the canonical ad-hoc shape.
    fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }
}

struct UnrelatedFlowReport {
    entries: Vec<Entry>,
}

impl UnrelatedFlowReport {
    // False-positive boundary: mentioning the input does not make another loop consume it.
    fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let _ = entries;
        let mut report = Self {
            entries: Vec::new(),
        };
        for entry in Vec::<Entry>::new() {
            report.entries.push(entry);
        }
        report
    }

    // False-positive boundary: same-typed local storage is not target-owned storage.
    fn create_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut buffer = Vec::new();
        buffer.extend(entries);
        Self {
            entries: Vec::new(),
        }
    }
}

struct GeneratedReport {
    entries: Vec<Entry>,
}

impl GeneratedReport {
    // False-positive boundary: a scalar controls generation but does not supply stored items.
    fn create_entries(count: u64) -> Self {
        let mut report = Self {
            entries: Vec::new(),
        };
        report.entries.extend((0..count).map(Entry));
        report
    }
}

struct MappedReport {
    entries: Vec<Entry>,
}

impl MappedReport {
    // False-positive boundary: transforming every item is policy beyond `FromIterator`.
    fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut report = Self {
            entries: Vec::new(),
        };
        report
            .entries
            .extend(entries.into_iter().map(|entry| Entry(entry.0 + 1)));
        report
    }
}

fn main() {}
