use std::ops::{Index, IndexMut};

const CELL_MAX: usize = 4;

#[derive(Default, Clone)]
pub struct GridCell<T>
where
    T: Clone + Copy + Default,
{
    pub len: usize,
    pub elements: [T; CELL_MAX],
}

impl<T> GridCell<T>
where
    T: Clone + Copy + Default,
{
    pub fn push(&mut self, elem: T) {
        if self.len < CELL_MAX {
            self.elements[self.len] = elem;
            self.len += 1;
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.elements[0..self.len].iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<T> {
        self.elements[0..self.len].iter_mut()
    }
}

#[derive(Clone)]
pub struct Grid<T>
where
    T: Clone + Copy + Default,
{
    pub width: usize,
    pub height: usize,
    grid: Vec<GridCell<T>>,
}

impl<T> Index<(usize, usize)> for Grid<T>
where
    T: Clone + Copy + Default,
{
    type Output = GridCell<T>;
    fn index(&self, (i, j): (usize, usize)) -> &Self::Output {
        let ind = i * self.height + j;
        &self.grid[ind]
    }
}

impl<T> IndexMut<(usize, usize)> for Grid<T>
where
    T: Clone + Copy + Default,
{
    fn index_mut(&mut self, (i, j): (usize, usize)) -> &mut Self::Output {
        let ind = i * self.height + j;
        &mut self.grid[ind]
    }
}

impl<T> Grid<T>
where
    T: Clone + Copy + Default,
{
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            grid: vec![GridCell::<T>::default(); width * height],
        }
    }

    pub fn clear(&mut self) {
        for cell in self.grid.iter_mut() {
            cell.clear()
        }
    }

    pub fn push(&mut self, ind: (usize, usize), value: T) {
        self[ind].push(value);
    }
}