//! Order and inverse of a playlist's changes (#382), without network.
//!
//! The changes chosen in « Clean up » refer to the positions of the list read. They are
//! applied from the last position to the first: removing a track only shifts those that
//! follow it, already handled. The inverse of an applied sequence is the inverse of each
//! operation, from the last to the first: the positions are exact there.

use crate::tidal::models::ListOp;

/// Changes chosen at most, like `MAX_ADD` for an add.
pub const MAX_OPS: usize = 10_000;

pub fn index(op: &ListOp) -> usize {
    match *op {
        ListOp::Remove { index, .. }
        | ListOp::Insert { index, .. }
        | ListOp::Replace { index, .. } => index,
    }
}

/// Cleanup changes, arranged to be applied: removals and replacements only,
/// one position each, from the last to the first.
pub fn forward(mut ops: Vec<ListOp>) -> Result<Vec<ListOp>, &'static str> {
    if ops.len() > MAX_OPS {
        return Err("too many changes");
    }
    if ops.iter().any(|op| matches!(op, ListOp::Insert { .. })) {
        return Err("unexpected operation");
    }
    ops.sort_by_key(|op| std::cmp::Reverse(index(op)));
    if ops.windows(2).any(|w| index(&w[0]) == index(&w[1])) {
        return Err("two changes at the same position");
    }
    Ok(ops)
}

pub fn inverse(op: ListOp) -> ListOp {
    match op {
        ListOp::Remove { index, track_id } => ListOp::Insert { index, track_id },
        ListOp::Insert { index, track_id } => ListOp::Remove { index, track_id },
        ListOp::Replace {
            index,
            track_id,
            with,
        } => ListOp::Replace {
            index,
            track_id: with,
            with: track_id,
        },
    }
}

/// What undoes `applied` (in the order they were applied).
pub fn undo(applied: &[ListOp]) -> Vec<ListOp> {
    applied.iter().rev().copied().map(inverse).collect()
}

fn with_index(op: ListOp, index: usize) -> ListOp {
    match op {
        ListOp::Remove { track_id, .. } => ListOp::Remove { index, track_id },
        ListOp::Insert { track_id, .. } => ListOp::Insert { index, track_id },
        ListOp::Replace { track_id, with, .. } => ListOp::Replace {
            index,
            track_id,
            with,
        },
    }
}

/// Operations skipped during an undo: the service refuses to add back a track removed
/// from the catalog (404, checked on 2026-11-12). A skipped add is missing from the list: the
/// following operations placed after it move up one place. A skipped replacement shifts
/// nothing.
#[derive(Debug, Default)]
pub struct Skips {
    /// Positions (original) of the skipped adds.
    inserts: Vec<usize>,
    pub count: usize,
}

impl Skips {
    /// `op` adjusted to the real list.
    pub fn adjust(&self, op: ListOp) -> ListOp {
        let i = index(&op);
        let shift = self.inserts.iter().filter(|&&k| k < i).count();
        with_index(op, i - shift)
    }

    /// `op` (original position) was skipped.
    pub fn note(&mut self, op: &ListOp) {
        if let ListOp::Insert { index, .. } = *op {
            self.inserts.push(index);
        }
        self.count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::TrackId;

    /// Applies the operations to a local list, as the API would.
    fn apply(list: &mut Vec<u64>, ops: &[ListOp]) {
        for op in ops {
            match *op {
                ListOp::Remove { index, track_id } => {
                    assert_eq!(list[index], track_id.0, "position checked");
                    list.remove(index);
                }
                ListOp::Insert { index, track_id } => {
                    list.insert(index.min(list.len()), track_id.0);
                }
                ListOp::Replace {
                    index,
                    track_id,
                    with,
                } => {
                    assert_eq!(list[index], track_id.0, "position checked");
                    list[index] = with.0;
                }
            }
        }
    }

    fn remove(index: usize, id: u64) -> ListOp {
        ListOp::Remove {
            index,
            track_id: TrackId(id),
        }
    }

    fn replace(index: usize, id: u64, with: u64) -> ListOp {
        ListOp::Replace {
            index,
            track_id: TrackId(id),
            with: TrackId(with),
        }
    }

    #[test]
    fn forward_then_undo_restores_the_list() {
        let original = vec![10, 11, 12, 13, 14, 15];
        let chosen = vec![
            remove(1, 11),
            replace(2, 12, 99),
            remove(3, 13),
            remove(5, 15),
        ];
        let ops = forward(chosen).unwrap();
        let mut list = original.clone();
        apply(&mut list, &ops);
        assert_eq!(list, vec![10, 99, 14]);
        apply(&mut list, &undo(&ops));
        assert_eq!(list, original);
    }

    #[test]
    fn partial_undo() {
        let original = vec![1, 2, 3, 4];
        let ops = forward(vec![remove(1, 2), remove(2, 3)]).unwrap();
        let mut list = original.clone();
        // Only the first operation succeeded.
        apply(&mut list, &ops[..1]);
        apply(&mut list, &undo(&ops[..1]));
        assert_eq!(list, original);
    }

    /// Undo where the tracks of `gone` can no longer be added, as in
    /// `apply_playlist_ops`.
    fn undo_with_gone(list: &mut Vec<u64>, ops: &[ListOp], gone: &[u64]) -> usize {
        let mut skips = Skips::default();
        for &op in ops {
            let adjusted = skips.adjust(op);
            let refused = match adjusted {
                ListOp::Insert { track_id, .. } => gone.contains(&track_id.0),
                ListOp::Replace { with, .. } => gone.contains(&with.0),
                ListOp::Remove { .. } => false,
            };
            if refused {
                skips.note(&op);
            } else {
                apply(list, &[adjusted]);
            }
        }
        skips.count
    }

    #[test]
    fn undo_skips_tracks_that_cannot_come_back() {
        // 20 and 40 are unavailable: 20 removed, 40 replaced by 99.
        let original = vec![10, 20, 30, 40, 50, 60];
        let chosen = vec![
            remove(1, 20),
            remove(2, 30),
            replace(3, 40, 99),
            remove(5, 60),
        ];
        let ops = forward(chosen).unwrap();
        let mut list = original.clone();
        apply(&mut list, &ops);
        assert_eq!(list, vec![10, 99, 50]);
        let skipped = undo_with_gone(&mut list, &undo(&ops), &[20, 40]);
        assert_eq!(skipped, 2);
        assert_eq!(
            list,
            vec![10, 30, 99, 50, 60],
            "the rest goes back to its place"
        );
    }

    #[test]
    fn forward_rejects_bad_plans() {
        assert!(forward(vec![remove(1, 2), replace(1, 2, 3)]).is_err());
        let insert = ListOp::Insert {
            index: 0,
            track_id: TrackId(1),
        };
        assert!(forward(vec![insert]).is_err());
        assert!(forward(vec![remove(0, 1); MAX_OPS + 1]).is_err());
        assert_eq!(forward(Vec::new()), Ok(Vec::new()));
    }

    #[test]
    fn ops_from_json() {
        let op: ListOp =
            serde_json::from_str(r#"{"op":"replace","index":3,"trackId":5,"with":6}"#).unwrap();
        assert_eq!(op, replace(3, 5, 6));
    }
}
