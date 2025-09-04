use wasm_bindgen::prelude::*;
use rand::prelude::*;
use std::fmt;

// Import console.log for debugging
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Macro for console logging from Rust
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct ParticleGrid {
    size: usize,
    num_types: usize,
    density: f32,
    radius: usize,
    type_grid: Vec<Vec<u8>>,
    affinity: Vec<Vec<i8>>,
    copy_type: Vec<u8>,
    replace_type: Vec<u8>,
    colors: Vec<[f32; 3]>,
    rng: ThreadRng,
}

#[wasm_bindgen]
impl ParticleGrid {
    #[wasm_bindgen(constructor)]
    pub fn new(
        size: usize, 
        num_types: usize, 
        density: f32, 
        radius: usize, 
        affinity_array: Option<Vec<i32>>
    ) -> ParticleGrid {
        console_log!("Creating ParticleGrid: {}x{}, {} types, density {:.2}, radius {}", 
            size, size, num_types, density, radius);
            
        let mut rng = thread_rng();

        // Initialize grid with random particles
        let mut type_grid = vec![vec![0u8; size]; size];
        for x in 0..size {
            for y in 0..size {
                if rng.gen::<f32>() < density {
                    type_grid[x][y] = rng.gen_range(1..=num_types as u8);
                }
            }
        }

        // Initialize affinity matrix
        let mut affinity = vec![vec![0i8; num_types + 1]; num_types + 1];
        if let Some(aff_array) = affinity_array {
            if aff_array.len() >= (num_types + 1) * (num_types + 1) {
                let mut idx = 0;
                for t in 0..=num_types {
                    for u in 0..=num_types {
                        affinity[t][u] = aff_array[idx] as i8;
                        idx += 1;
                    }
                }
                console_log!("Used custom affinity matrix");
            } else {
                console_log!("Custom affinity array too small, using random");
                Self::randomize_affinity(&mut affinity, num_types, &mut rng);
            }
        } else {
            Self::randomize_affinity(&mut affinity, num_types, &mut rng);
        }

        // Initialize copy_type array
        let mut copy_type = vec![0u8; num_types + 1];
        for t in 0..=num_types {
            let choices: Vec<u8> = (1..=num_types as u8)
                .filter(|&c| c != t as u8)
                .collect();
            copy_type[t] = *choices
                .choose(&mut rng)
                .unwrap_or(&((t as u8 % num_types as u8) + 1));
        }

        // Initialize replace_type array
        let mut replace_type = vec![0u8; num_types + 1];
        for t in 0..=num_types {
            let choices: Vec<u8> = (1..=num_types as u8)
                .filter(|&c| c != t as u8 && c != copy_type[t])
                .collect();
            if choices.is_empty() {
                replace_type[t] = if num_types >= 1 { 1 } else { 0 };
            } else {
                replace_type[t] = *choices.choose(&mut rng).unwrap();
            }
        }

        // Initialize color palette
        let mut colors = vec![[0.1, 0.1, 0.1]; num_types + 1];
        for t in 1..=num_types {
            let h = (t as f32 - 1.0) / (num_types as f32);
            colors[t] = Self::hsv_to_rgb(h, 0.8, 1.0);
        }

        console_log!("ParticleGrid initialized successfully");

        ParticleGrid {
            size,
            num_types,
            density,
            radius,
            type_grid,
            affinity,
            copy_type,
            replace_type,
            colors,
            rng,
        }
    }

    fn randomize_affinity(affinity: &mut Vec<Vec<i8>>, num_types: usize, rng: &mut ThreadRng) {
        for t in 0..=num_types {
            for u in 0..=num_types {
                affinity[t][u] = if rng.gen_bool(0.5) { 1 } else { -1 };
            }
        }
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
        let h6 = (h * 6.0).fract();
        let i = (h * 6.0).floor() as i32 % 6;
        let f = h6;
        let p = v * (1.0 - s);
        let q = v * (1.0 - f * s);
        let t = v * (1.0 - (1.0 - f) * s);
        
        match i {
            0 => [v, t, p],
            1 => [q, v, p],
            2 => [p, v, t],
            3 => [p, q, v],
            4 => [t, p, v],
            _ => [v, p, q],
        }
    }

    #[inline]
    fn inside(&self, x: isize, y: isize) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.size && (y as usize) < self.size
    }

    fn try_replace_particle(&mut self, x: usize, y: usize) {
        let p_type = self.type_grid[x][y];
        if p_type == 0 {
            return;
        }

        let ct = self.copy_type[p_type as usize];
        let rt = self.replace_type[p_type as usize];

        // Look for copy_type neighbor
        let mut has_copy_neighbor = false;
        for j in (y.saturating_sub(1))..=((y + 1).min(self.size - 1)) {
            for i in (x.saturating_sub(1))..=((x + 1).min(self.size - 1)) {
                if self.type_grid[i][j] == ct {
                    has_copy_neighbor = true;
                    break;
                }
            }
            if has_copy_neighbor {
                break;
            }
        }

        if !has_copy_neighbor {
            return;
        }

        // Replace all rt with ct in neighborhood
        for j in (y.saturating_sub(1))..=((y + 1).min(self.size - 1)) {
            for i in (x.saturating_sub(1))..=((x + 1).min(self.size - 1)) {
                if self.type_grid[i][j] == rt {
                    self.type_grid[i][j] = ct;
                }
            }
        }
    }

    fn score_within_radius(&mut self, x: usize, y: usize) -> (usize, usize) {
        let p_type = self.type_grid[x][y];
        let mut best: f32 = -1_000_000.0;
        let mut tiebreak: Vec<(usize, usize)> = vec![(x, y)];

        // Check adjacent empty cells
        for j in (y.saturating_sub(1))..=((y + 1).min(self.size - 1)) {
            for i in (x.saturating_sub(1))..=((x + 1).min(self.size - 1)) {
                if self.type_grid[i][j] != 0 {
                    continue;
                }

                let mut score = 0i32;
                let mut cell_count = 0i32;

                // Calculate bounds for scoring region
                let rx0 = i.saturating_sub(self.radius);
                let rx1 = (i + self.radius).min(self.size - 1);
                let ry0 = j.saturating_sub(self.radius);
                let ry1 = (j + self.radius).min(self.size - 1);

                // Score calculation
                for yy in ry0..=ry1 {
                    for xx in rx0..=rx1 {
                        cell_count += 1;
                        let ct = self.type_grid[xx][yy];
                        if ct != 0 {
                            let a = self.affinity[p_type as usize][ct as usize];
                            score += if a == 1 { 1 } else { -1 };
                        }
                    }
                }

                let norm = score as f32 / (cell_count as f32).max(1.0);
                if norm > best {
                    best = norm;
                    tiebreak.clear();
                    tiebreak.push((i, j));
                } else if (norm - best).abs() < f32::EPSILON {
                    tiebreak.push((i, j));
                }
            }
        }

        if tiebreak.is_empty() {
            (x, y)
        } else {
            *tiebreak.choose(&mut self.rng).unwrap()
        }
    }

    fn move_particle(&mut self, x: usize, y: usize) {
        let p_type = self.type_grid[x][y];
        if p_type == 0 {
            return;
        }

        let (bx, by) = self.score_within_radius(x, y);
        if bx == x && by == y {
            return;
        }

        self.type_grid[bx][by] = p_type;
        self.type_grid[x][y] = 0;
    }

    #[wasm_bindgen]
    pub fn step(&mut self) {
        let total_cells = (self.size * self.size) as f32;
        let updates = (0.2 * self.density * total_cells).floor() as usize;

        if updates == 0 {
            return;
        }

        // Collect current non-empty cells
        let mut particles: Vec<(usize, usize)> = Vec::new();
        particles.reserve(self.size * self.size / 2);
        
        for x in 0..self.size {
            for y in 0..self.size {
                if self.type_grid[x][y] != 0 {
                    particles.push((x, y));
                }
            }
        }

        if particles.is_empty() {
            return;
        }

        // Update random particles
        for _ in 0..updates {
            if particles.is_empty() {
                break;
            }
            let idx = self.rng.gen_range(0..particles.len());
            let (x, y) = particles[idx];
            
            // Check if particle still exists
            if self.type_grid[x][y] == 0 {
                particles.swap_remove(idx);
                continue;
            }

            self.try_replace_particle(x, y);
            self.move_particle(x, y);
        }
    }

    #[wasm_bindgen]
    pub fn export_grid(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.size * self.size);
        for y in 0..self.size {
            for x in 0..self.size {
                data.push(self.type_grid[x][y]);
            }
        }
        data
    }

    // Getters for JavaScript
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.size
    }

    #[wasm_bindgen(getter)]
    pub fn num_types(&self) -> usize {
        self.num_types
    }

    #[wasm_bindgen(getter)]
    pub fn density(&self) -> f32 {
        self.density
    }

    #[wasm_bindgen(getter)]
    pub fn radius(&self) -> usize {
        self.radius
    }

    // Debug method
    #[wasm_bindgen]
    pub fn debug_info(&self) -> String {
        format!(
            "Grid {}x{}, {} types, density {:.2}, radius {}, particles: {}",
            self.size,
            self.size,
            self.num_types,
            self.density,
            self.radius,
            self.count_particles()
        )
    }

    fn count_particles(&self) -> usize {
        let mut count = 0;
        for x in 0..self.size {
            for y in 0..self.size {
                if self.type_grid[x][y] != 0 {
                    count += 1;
                }
            }
        }
        count
    }
}

#[wasm_bindgen]
impl ParticleGrid {
    #[wasm_bindgen]
    pub fn update_affinity(&mut self, new_affinity: Vec<i32>) {
        if new_affinity.len() >= (self.num_types + 1) * (self.num_types + 1) {
            let mut idx = 0;
            for t in 0..=self.num_types {
                for u in 0..=self.num_types {
                    self.affinity[t][u] = new_affinity[idx] as i8;
                    idx += 1;
                }
            }
        }
    }
    
    #[wasm_bindgen]
    pub fn update_copy_replace(&mut self, copy_types: Vec<u8>, replace_types: Vec<u8>) {
        if copy_types.len() > self.num_types && replace_types.len() > self.num_types {
            self.copy_type = copy_types;
            self.replace_type = replace_types;
        }
    }
}