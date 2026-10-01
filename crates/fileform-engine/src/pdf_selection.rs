// SPDX-License-Identifier: Apache-2.0
use crate::{fail, PdfPageSelection, Result};
use serde::Deserialize;

/// Positions refer to the ordered, concatenated input pages, starting at one.
/// Output order and duplicate positions are intentional.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum SplitSelection {
    Ranges { ranges: String },
    Every { count: u32 },
    After { positions: Vec<u32> },
}
fn invalid() -> crate::Failure {
    fail("invalid_request", "Use existing one-based page numbers or increasing ranges separated by commas; semicolons separate output PDFs.")
}
pub(crate) fn ranges(value: &str, page_count: usize) -> Result<Vec<Vec<usize>>> {
    if value.len() > 16384 {
        return Err(fail("limit", "Page selection exceeds 16384 bytes."));
    }
    if page_count == 0 || value.trim().is_empty() {
        return Err(invalid());
    }
    let mut total = 0usize;
    value
        .split(';')
        .map(|group| {
            let mut selected = Vec::new();
            for part in group.split(',') {
                let bounds = part.split('-').map(str::trim).collect::<Vec<_>>();
                let number = |v: &str| -> Result<usize> {
                    if v.is_empty() || !v.bytes().all(|c| c.is_ascii_digit()) {
                        return Err(invalid());
                    }
                    v.parse::<usize>().map_err(|_| invalid())
                };
                if !(1..=2).contains(&bounds.len()) {
                    return Err(invalid());
                }
                let start = number(bounds[0])?;
                let end = if bounds.len() == 2 {
                    number(bounds[1])?
                } else {
                    start
                };
                if start == 0 || end < start || end > page_count {
                    return Err(invalid());
                }
                let length = end - start + 1;
                if length > 1000 - total {
                    return Err(fail(
                        "limit",
                        "Select at most 1000 pages including duplicates.",
                    ));
                }
                total += length;
                selected.extend(start - 1..end);
            }
            Ok(selected)
        })
        .collect()
}
pub(crate) fn pages(value: &str, counts: &[usize]) -> Result<Vec<PdfPageSelection>> {
    let count: usize = counts.iter().sum();
    if count == 0 || count > 128_000 || value.contains(';') {
        return Err(fail(
            "invalid_request",
            "Use one comma-separated page selection from at most 128000 source pages.",
        ));
    }
    let groups = ranges(value, count)?;
    Ok(groups
        .into_iter()
        .flatten()
        .map(|position| {
            let mut offset = position;
            for (source_index, &count) in counts.iter().enumerate() {
                if offset < count {
                    return PdfPageSelection {
                        source_index,
                        page_index: offset as u32,
                        clockwise_rotation: 0,
                    };
                }
                offset -= count;
            }
            unreachable!("validated page position")
        })
        .collect())
}
pub(crate) fn select(
    selection: &SplitSelection,
    pages: &[PdfPageSelection],
) -> Result<Vec<Vec<PdfPageSelection>>> {
    if pages.is_empty() || pages.len() > 128_000 {
        return Err(fail(
            "limit",
            "Source page inventory is empty or exceeds 128000 pages.",
        ));
    }
    let indices = match selection {
        SplitSelection::Ranges { ranges: value } => ranges(value, pages.len())?,
        SplitSelection::Every { count } => {
            if *count == 0 {
                return Err(fail(
                    "invalid_request",
                    "Choose a positive number of pages per PDF.",
                ));
            }
            if pages.len() > 1000 {
                return Err(fail("limit", "Split at most 1000 pages per job."));
            }
            (0..pages.len())
                .step_by((*count as usize).min(pages.len()))
                .map(|start| {
                    (start..start.saturating_add(*count as usize).min(pages.len())).collect()
                })
                .collect()
        }
        SplitSelection::After { positions } => {
            if pages.len() > 1000 || positions.len() > 1000 {
                return Err(fail("limit", "Split at most 1000 pages per job."));
            }
            let mut previous = 0;
            let mut indices = Vec::new();
            for &position in positions {
                let position = position as usize;
                if position <= previous || position >= pages.len() {
                    return Err(fail("invalid_request", "Split markers must be unique increasing page positions before the last page."));
                }
                indices.push((previous..position).collect());
                previous = position;
            }
            indices.push((previous..pages.len()).collect());
            indices
        }
    };
    Ok(indices
        .into_iter()
        .map(|group: Vec<usize>| {
            group
                .into_iter()
                .map(|index| pages[index].clone())
                .collect()
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_range_order_duplicates_boundaries_and_limits() {
        assert_eq!(
            ranges("3, 1-2,3;2", 3).unwrap(),
            vec![vec![2, 0, 1, 2], vec![1]]
        );
        for value in [
            "",
            "0",
            "4",
            "2-1",
            "1,,2",
            "1;",
            ";1",
            "1-2-3",
            "-1",
            "+1",
            "1.0",
            "99999999999999999999999",
        ] {
            assert!(ranges(value, 3).is_err(), "{value}");
        }
        assert!(ranges("1-1001", 1001).is_err());
        assert_eq!(ranges("1-1000", 1000).unwrap()[0].len(), 1000);
        let mapped = pages("3,1-2,3", &[2, 1]).unwrap();
        assert_eq!(
            mapped
                .iter()
                .map(|v| (v.source_index, v.page_index))
                .collect::<Vec<_>>(),
            vec![(1, 0), (0, 0), (0, 1), (1, 0)]
        );
        assert!(pages("1;2", &[2]).is_err());
        let pages = (0..5)
            .map(|page_index| PdfPageSelection {
                source_index: 0,
                page_index,
                clockwise_rotation: 90,
            })
            .collect::<Vec<_>>();
        let selected = select(&SplitSelection::Every { count: 2 }, &pages).unwrap();
        assert_eq!(
            selected.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![2, 2, 1]
        );
        assert_eq!(selected[0][0].clockwise_rotation, 90);
        let marked = select(
            &SplitSelection::After {
                positions: vec![1, 3],
            },
            &pages,
        )
        .unwrap();
        assert_eq!(
            marked.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![1, 2, 2]
        );
        assert_eq!(
            select(&SplitSelection::Every { count: u32::MAX }, &pages)
                .unwrap()
                .len(),
            1
        );
        for positions in [vec![0], vec![5], vec![2, 2], vec![3, 1]] {
            assert!(select(&SplitSelection::After { positions }, &pages).is_err());
        }
        assert!(select(&SplitSelection::Every { count: 0 }, &pages).is_err());
    }
}
