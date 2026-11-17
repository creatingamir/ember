//! Removal of blocks that nothing jumps to.

use crate::{Block, BlockId, Terminator};

/// Drops the blocks that cannot be reached from the entry and numbers the rest again, in order.
///
/// Fails with a description when a terminator points at a block that does not exist.
pub(crate) fn remove_unreachable(blocks: Vec<Block>) -> Result<Vec<Block>, &'static str> {
    let reachable = find_reachable(&blocks);
    let new_ids = new_block_numbers(&reachable);

    let mut kept = Vec::new();
    for (block, is_reachable) in blocks.into_iter().zip(reachable) {
        if is_reachable {
            kept.push(renumber(block, &new_ids)?);
        }
    }
    Ok(kept)
}

/// Marks every block that can be reached from the entry block by following terminators.
fn find_reachable(blocks: &[Block]) -> Vec<bool> {
    let mut reachable = vec![false; blocks.len()];
    let mut pending = vec![BlockId(0)];
    while let Some(id) = pending.pop() {
        let index = id.0 as usize;
        // A target that does not exist is skipped here and reported when its jump is renumbered.
        let Some(seen) = reachable.get_mut(index) else {
            continue;
        };
        if *seen {
            continue;
        }
        *seen = true;

        if let Some(block) = blocks.get(index) {
            pending.extend(successors(&block.term));
        }
    }
    reachable
}

/// Returns the blocks a terminator can continue in.
fn successors(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Jump(target) => vec![*target],
        Terminator::Branch {
            then_to, else_to, ..
        } => vec![*then_to, *else_to],
        Terminator::Return(_) | Terminator::Unreachable => Vec::new(),
    }
}

/// Returns, for every old block, the number it has once the unreachable blocks are gone.
fn new_block_numbers(reachable: &[bool]) -> Vec<u32> {
    let mut numbers = Vec::new();
    let mut next = 0;
    for &is_reachable in reachable {
        numbers.push(next);
        if is_reachable {
            next += 1;
        }
    }
    numbers
}

/// Gives a block its new number and points its terminator at the new numbers.
fn renumber(block: Block, new_ids: &[u32]) -> Result<Block, &'static str> {
    Ok(Block {
        id: renumber_id(block.id, new_ids)?,
        insts: block.insts,
        term: renumber_terminator(block.term, new_ids)?,
    })
}

/// Returns the new number of a block.
fn renumber_id(id: BlockId, new_ids: &[u32]) -> Result<BlockId, &'static str> {
    match new_ids.get(id.0 as usize) {
        Some(number) => Ok(BlockId(*number)),
        None => Err("a jump points at a block that does not exist"),
    }
}

/// Points the targets of a terminator at the new block numbers.
fn renumber_terminator(term: Terminator, new_ids: &[u32]) -> Result<Terminator, &'static str> {
    match term {
        Terminator::Jump(target) => Ok(Terminator::Jump(renumber_id(target, new_ids)?)),
        Terminator::Branch {
            cond,
            then_to,
            else_to,
        } => Ok(Terminator::Branch {
            cond,
            then_to: renumber_id(then_to, new_ids)?,
            else_to: renumber_id(else_to, new_ids)?,
        }),
        Terminator::Return(_) | Terminator::Unreachable => Ok(term),
    }
}
