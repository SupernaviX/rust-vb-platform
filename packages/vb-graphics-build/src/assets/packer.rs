use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct InputRegion {
    pub name: String,
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OutputRegion {
    pub bgmap: u8,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}
impl OutputRegion {
    fn overlaps(self, other: Self) -> bool {
        self.bgmap == other.bgmap
            && self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

#[derive(Debug, Clone)]
pub struct Packer {
    state: PackerState,
}

impl Packer {
    pub fn new(bgmap_start: u8) -> Self {
        Self {
            state: PackerState::new(bgmap_start),
        }
    }
    pub fn pack(
        &mut self,
        mut unplaced_regions: Vec<InputRegion>,
    ) -> BTreeMap<String, OutputRegion> {
        let mut result = BTreeMap::new();
        unplaced_regions.sort_by_key(|r| std::cmp::Reverse(r.width * r.height));
        for unplaced in unplaced_regions {
            let region = self.state.place(unplaced.width, unplaced.height);
            result.insert(unplaced.name, region);
        }
        result
    }
}

#[derive(Debug, Clone)]
struct PackerState {
    open: Vec<OutputRegion>,
    next_bgmap: u8,
}
impl PackerState {
    fn new(bgmap_start: u8) -> Self {
        Self {
            open: vec![],
            next_bgmap: bgmap_start,
        }
    }
    fn place(&mut self, width: usize, height: usize) -> OutputRegion {
        let mut best: Option<(usize, usize)> = None;
        for (index, rect) in self.open.iter().enumerate() {
            if rect.width < width || rect.height < height {
                continue;
            }
            let area = rect.width * rect.height;
            if best.is_none_or(|(_, best_area)| best_area > area) {
                best = Some((index, area));
            }
        }
        if let Some((index, _)) = best {
            let rect = self.open[index];
            let result = OutputRegion {
                bgmap: rect.bgmap,
                x: rect.x,
                y: rect.y,
                width,
                height,
            };
            let mut new_areas = vec![];
            self.open.retain(|r| {
                if !r.overlaps(result) {
                    return true;
                }
                if r.x < result.x {
                    new_areas.push(OutputRegion {
                        width: result.x - r.x,
                        ..*r
                    });
                }
                if r.x + r.width > result.x + result.width {
                    new_areas.push(OutputRegion {
                        x: result.x + result.width,
                        width: r.x + r.width - (result.x + result.width),
                        ..*r
                    });
                }
                if r.y < result.y {
                    new_areas.push(OutputRegion {
                        height: result.y - r.y,
                        ..*r
                    });
                }
                if r.y + r.height > result.y + result.height {
                    new_areas.push(OutputRegion {
                        y: result.y + result.height,
                        height: r.y + r.height - (result.y + result.height),
                        ..*r
                    });
                }
                false
            });
            self.open.append(&mut new_areas);
            result
        } else {
            let result = OutputRegion {
                bgmap: self.next_bgmap,
                x: 0,
                y: 0,
                width,
                height,
            };
            if width < 512 {
                self.open.push(OutputRegion {
                    bgmap: self.next_bgmap,
                    x: width,
                    y: 0,
                    width: 512 - width,
                    height: 512,
                });
            }
            if height < 512 {
                self.open.push(OutputRegion {
                    bgmap: self.next_bgmap,
                    x: 0,
                    y: height,
                    width: 512,
                    height: 512 - height,
                });
            }
            self.next_bgmap += 1;
            result
        }
    }
}
