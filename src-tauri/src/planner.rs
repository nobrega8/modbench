//! Groups the points to read into as few Modbus requests as possible.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Area {
    Coil,
    Discrete,
    Input,
    Holding,
}

impl Area {
    pub fn is_bit(self) -> bool {
        matches!(self, Area::Coil | Area::Discrete)
    }
}

/// One thing to read: `count` consecutive addresses starting at `address`
/// (bits for coil/discrete, registers otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub area: Area,
    pub address: u16,
    pub count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLimits {
    /// Largest register block per request (FC03/FC04). Modbus allows 125.
    pub max_registers: u16,
    /// Largest bit block per request (FC01/FC02). Modbus allows 2000.
    pub max_bits: u16,
    /// Unused addresses allowed between two points in the same block. Larger
    /// gaps mean fewer requests, but some devices reject reads that touch
    /// unmapped addresses (exception 02), so the default is 0.
    pub max_gap: u16,
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            max_registers: 125,
            max_bits: 2000,
            max_gap: 0,
        }
    }
}

/// One request, and which input points it covers (by index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub area: Area,
    pub address: u16,
    pub count: u16,
    pub points: Vec<usize>,
}

impl Block {
    fn end(&self) -> u32 {
        self.address as u32 + self.count as u32
    }

    /// Slice of this block's data that belongs to a point.
    pub fn range_of(&self, point: &Point) -> std::ops::Range<usize> {
        let start = (point.address - self.address) as usize;
        start..start + point.count as usize
    }
}

/// Builds the read plan. Points with `count == 0` are treated as 1. Points
/// larger than the area's block limit get a block of their own, so the
/// device can report the error for that point alone.
pub fn plan(points: &[Point], limits: PlanLimits) -> Vec<Block> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by_key(|&i| (points[i].area, points[i].address, points[i].count));

    let mut blocks: Vec<Block> = Vec::new();
    for i in order {
        let p = points[i];
        let count = p.count.max(1);
        let start = p.address as u32;
        let end = start + count as u32;
        let max = if p.area.is_bit() {
            limits.max_bits
        } else {
            limits.max_registers
        }
        .max(1) as u32;

        if let Some(last) = blocks.last_mut() {
            let fits = last.area == p.area
                && start <= last.end() + limits.max_gap as u32
                && end.max(last.end()) - last.address as u32 <= max;
            if fits {
                last.count = (end.max(last.end()) - last.address as u32) as u16;
                last.points.push(i);
                continue;
            }
        }
        blocks.push(Block {
            area: p.area,
            address: p.address,
            count,
            points: vec![i],
        });
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(area: Area, address: u16, count: u16) -> Point {
        Point {
            area,
            address,
            count,
        }
    }

    fn spans(blocks: &[Block]) -> Vec<(Area, u16, u16)> {
        blocks
            .iter()
            .map(|b| (b.area, b.address, b.count))
            .collect()
    }

    #[test]
    fn merges_contiguous_points() {
        let points = [
            pt(Area::Holding, 10, 2),
            pt(Area::Holding, 12, 2),
            pt(Area::Holding, 14, 1),
        ];
        let blocks = plan(&points, PlanLimits::default());
        assert_eq!(spans(&blocks), [(Area::Holding, 10, 5)]);
        assert_eq!(blocks[0].points, [0, 1, 2]);
        assert_eq!(blocks[0].range_of(&points[1]), 2..4);
    }

    #[test]
    fn sorts_input_and_keeps_original_indexes() {
        let points = [pt(Area::Holding, 20, 1), pt(Area::Holding, 19, 1)];
        let blocks = plan(&points, PlanLimits::default());
        assert_eq!(spans(&blocks), [(Area::Holding, 19, 2)]);
        assert_eq!(blocks[0].points, [1, 0]);
    }

    #[test]
    fn splits_on_gaps_unless_allowed() {
        let points = [pt(Area::Input, 0, 2), pt(Area::Input, 4, 2)];
        assert_eq!(
            spans(&plan(&points, PlanLimits::default())),
            [(Area::Input, 0, 2), (Area::Input, 4, 2)]
        );
        let limits = PlanLimits {
            max_gap: 2,
            ..Default::default()
        };
        assert_eq!(spans(&plan(&points, limits)), [(Area::Input, 0, 6)]);
    }

    #[test]
    fn never_mixes_areas() {
        let points = [
            pt(Area::Holding, 0, 1),
            pt(Area::Input, 1, 1),
            pt(Area::Coil, 0, 1),
            pt(Area::Discrete, 1, 1),
        ];
        assert_eq!(plan(&points, PlanLimits::default()).len(), 4);
    }

    #[test]
    fn respects_block_limits() {
        let points: Vec<Point> = (0..130).map(|a| pt(Area::Holding, a, 1)).collect();
        assert_eq!(
            spans(&plan(&points, PlanLimits::default())),
            [(Area::Holding, 0, 125), (Area::Holding, 125, 5)]
        );

        let limits = PlanLimits {
            max_registers: 10,
            ..Default::default()
        };
        // A float32 that would cross the limit starts a new block.
        let points = [pt(Area::Holding, 0, 9), pt(Area::Holding, 9, 2)];
        assert_eq!(
            spans(&plan(&points, limits)),
            [(Area::Holding, 0, 9), (Area::Holding, 9, 2)]
        );

        let bits: Vec<Point> = (0..2001).map(|a| pt(Area::Coil, a, 1)).collect();
        assert_eq!(
            spans(&plan(&bits, PlanLimits::default())),
            [(Area::Coil, 0, 2000), (Area::Coil, 2000, 1)]
        );
    }

    #[test]
    fn overlapping_points_share_a_block() {
        // A whole status word plus single bits of it.
        let points = [
            pt(Area::Holding, 512, 1),
            pt(Area::Holding, 512, 1),
            pt(Area::Holding, 511, 2),
        ];
        let blocks = plan(&points, PlanLimits::default());
        assert_eq!(spans(&blocks), [(Area::Holding, 511, 2)]);
        assert_eq!(blocks[0].points.len(), 3);
    }

    #[test]
    fn oversized_point_gets_its_own_block() {
        let points = [pt(Area::Holding, 0, 1), pt(Area::Holding, 1, 200)];
        assert_eq!(
            spans(&plan(&points, PlanLimits::default())),
            [(Area::Holding, 0, 1), (Area::Holding, 1, 200)]
        );
    }

    #[test]
    fn top_of_address_space() {
        let points = [pt(Area::Holding, 65534, 1), pt(Area::Holding, 65535, 1)];
        assert_eq!(
            spans(&plan(&points, PlanLimits::default())),
            [(Area::Holding, 65534, 2)]
        );
    }
}
