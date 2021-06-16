use crate::timestamp::NaiveDateTime;
use ref_cast::RefCast;
use std::fmt::{self, Debug};
use std::iter::Copied;
use std::ops::{Div, Index};
use std::slice;

pub struct Matrix {
    queries: usize,
    rows: Vec<(NaiveDateTime, Vec<u32>)>,
}

#[derive(RefCast)]
#[repr(transparent)]
pub struct Row([u32]);

impl Matrix {
    pub(crate) fn new(queries: usize) -> Self {
        Matrix {
            queries,
            rows: Vec::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.queries
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn iter(&self) -> Iter {
        Iter(self.rows.iter())
    }

    pub(crate) fn push(&mut self, timestamp: NaiveDateTime, data: Vec<u32>) {
        self.rows.push((timestamp, data));
    }
}

impl<'a> IntoIterator for &'a Matrix {
    type Item = (NaiveDateTime, &'a Row);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct Iter<'a>(slice::Iter<'a, (NaiveDateTime, Vec<u32>)>);

impl<'a> Iterator for Iter<'a> {
    type Item = (NaiveDateTime, &'a Row);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|(timestamp, data)| (*timestamp, Row::ref_cast(data)))
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|(timestamp, data)| (*timestamp, Row::ref_cast(data)))
    }
}

impl Index<usize> for Row {
    type Output = u32;

    fn index(&self, i: usize) -> &Self::Output {
        &self.0[i]
    }
}

impl<'a> IntoIterator for &'a Row {
    type Item = u32;
    type IntoIter = Copied<slice::Iter<'a, u32>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().copied()
    }
}

pub struct RelativeRow<'a> {
    row: &'a Row,
    total: u32,
}

impl<'a> Div<u32> for &'a Row {
    type Output = RelativeRow<'a>;

    fn div(self, rhs: u32) -> Self::Output {
        RelativeRow {
            row: self,
            total: rhs,
        }
    }
}

impl Debug for Row {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.debug_list().entries(&self.0).finish()
    }
}

impl<'a> Debug for RelativeRow<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut list = formatter.debug_list();
        for value in self.row {
            list.entry(&(value as f32 / self.total as f32));
        }
        list.finish()
    }
}
