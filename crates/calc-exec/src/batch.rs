use crate::domain::Domain;

#[derive(Clone, Debug)]
enum Columns {
    F32(Vec<Vec<f32>>),
    F64(Vec<Vec<f64>>),
}

#[derive(Clone, Debug)]
pub struct Batch {
    columns: Columns,
    length: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchError {
    ChannelLengthMismatch {
        channel: usize,
        expected: usize,
        actual: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchShapeError {
    DomainMismatch { expected: Domain, actual: Domain },
    ChannelCountMismatch { expected: usize, actual: usize },
    LengthMismatch { expected: usize, actual: usize },
}

fn check_lengths<T>(length: usize, columns: &[Vec<T>]) -> Result<(), BatchError> {
    match columns
        .iter()
        .enumerate()
        .find(|(_, column)| column.len() != length)
    {
        Some((channel, column)) => Err(BatchError::ChannelLengthMismatch {
            channel,
            expected: length,
            actual: column.len(),
        }),
        None => Ok(()),
    }
}

impl Batch {
    pub fn from_f32_columns(length: usize, columns: Vec<Vec<f32>>) -> Result<Batch, BatchError> {
        check_lengths(length, &columns)?;
        Ok(Batch {
            columns: Columns::F32(columns),
            length,
        })
    }

    pub fn from_f64_columns(length: usize, columns: Vec<Vec<f64>>) -> Result<Batch, BatchError> {
        check_lengths(length, &columns)?;
        Ok(Batch {
            columns: Columns::F64(columns),
            length,
        })
    }

    pub fn zeroed(domain: Domain, channel_count: usize, length: usize) -> Batch {
        let columns = match domain {
            Domain::F32 => Columns::F32(vec![vec![0.0; length]; channel_count]),
            Domain::F64 => Columns::F64(vec![vec![0.0; length]; channel_count]),
        };
        Batch { columns, length }
    }

    pub fn domain(&self) -> Domain {
        match self.columns {
            Columns::F32(_) => Domain::F32,
            Columns::F64(_) => Domain::F64,
        }
    }

    pub fn channel_count(&self) -> usize {
        match &self.columns {
            Columns::F32(columns) => columns.len(),
            Columns::F64(columns) => columns.len(),
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn f32_channel(&self, channel: usize) -> Option<&[f32]> {
        match &self.columns {
            Columns::F32(columns) => columns.get(channel).map(Vec::as_slice),
            Columns::F64(_) => None,
        }
    }

    pub fn f64_channel(&self, channel: usize) -> Option<&[f64]> {
        match &self.columns {
            Columns::F64(columns) => columns.get(channel).map(Vec::as_slice),
            Columns::F32(_) => None,
        }
    }

    pub fn f32_channel_mut(&mut self, channel: usize) -> Option<&mut [f32]> {
        match &mut self.columns {
            Columns::F32(columns) => columns.get_mut(channel).map(Vec::as_mut_slice),
            Columns::F64(_) => None,
        }
    }

    pub fn f64_channel_mut(&mut self, channel: usize) -> Option<&mut [f64]> {
        match &mut self.columns {
            Columns::F64(columns) => columns.get_mut(channel).map(Vec::as_mut_slice),
            Columns::F32(_) => None,
        }
    }

    pub fn check_shape(
        &self,
        domain: Domain,
        channel_count: usize,
        length: usize,
    ) -> Result<(), BatchShapeError> {
        if self.domain() != domain {
            return Err(BatchShapeError::DomainMismatch {
                expected: domain,
                actual: self.domain(),
            });
        }
        if self.channel_count() != channel_count {
            return Err(BatchShapeError::ChannelCountMismatch {
                expected: channel_count,
                actual: self.channel_count(),
            });
        }
        if self.length != length {
            return Err(BatchShapeError::LengthMismatch {
                expected: length,
                actual: self.length,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_of_equal_length_form_a_batch() {
        let columns = vec![vec![1.0, -0.0], vec![f64::NAN, f64::INFINITY]];

        let batch = Batch::from_f64_columns(2, columns).unwrap();

        assert_eq!(batch.f64_channel(1).unwrap()[1], f64::INFINITY);
    }

    #[test]
    fn column_of_other_length_is_rejected() {
        let columns = vec![vec![1.0_f32, 2.0], vec![3.0]];

        let batch = Batch::from_f32_columns(2, columns);

        assert_eq!(
            batch.err(),
            Some(BatchError::ChannelLengthMismatch {
                channel: 1,
                expected: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn batch_without_channels_keeps_its_length() {
        let batch = Batch::from_f64_columns(7, Vec::new()).unwrap();

        let length = batch.len();

        assert_eq!(length, 7);
    }

    #[test]
    fn channel_of_other_domain_is_not_available() {
        let batch = Batch::zeroed(Domain::F32, 1, 3);

        let channel = batch.f64_channel(0);

        assert!(channel.is_none());
    }

    #[test]
    fn writing_through_mutable_channel_changes_the_batch() {
        let mut batch = Batch::zeroed(Domain::F32, 1, 3);

        batch.f32_channel_mut(0).unwrap()[2] = -0.0;

        assert_eq!(
            batch.f32_channel(0).unwrap()[2].to_bits(),
            (-0.0_f32).to_bits()
        );
    }

    #[test]
    fn shape_check_reports_domain_mismatch() {
        let batch = Batch::zeroed(Domain::F32, 1, 3);

        let checked = batch.check_shape(Domain::F64, 1, 3);

        assert_eq!(
            checked,
            Err(BatchShapeError::DomainMismatch {
                expected: Domain::F64,
                actual: Domain::F32
            })
        );
    }

    #[test]
    fn shape_check_reports_channel_count_mismatch() {
        let batch = Batch::zeroed(Domain::F64, 1, 3);

        let checked = batch.check_shape(Domain::F64, 2, 3);

        assert_eq!(
            checked,
            Err(BatchShapeError::ChannelCountMismatch {
                expected: 2,
                actual: 1
            })
        );
    }

    #[test]
    fn shape_check_reports_length_mismatch() {
        let batch = Batch::zeroed(Domain::F64, 1, 3);

        let checked = batch.check_shape(Domain::F64, 1, 4);

        assert_eq!(
            checked,
            Err(BatchShapeError::LengthMismatch {
                expected: 4,
                actual: 3
            })
        );
    }
}
