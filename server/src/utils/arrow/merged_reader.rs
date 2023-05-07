/*
 * Parseable Server (C) 2022 - 2023 Parseable, Inc.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 *
 */

use std::{fs::File, path::PathBuf};

use arrow_array::{RecordBatch, TimestampMillisecondArray};
use arrow_ipc::reader::StreamReader;
use arrow_schema::Schema;
use itertools::kmerge_by;

use super::adapt_batch;

#[derive(Debug)]
pub struct MergedRecordReader {
    pub readers: Vec<StreamReader<File>>,
}

impl MergedRecordReader {
    pub fn try_new(files: &[PathBuf]) -> Result<Self, ()> {
        let mut readers = Vec::with_capacity(files.len());

        for file in files {
            let reader = StreamReader::try_new(File::open(file).unwrap(), None).map_err(|_| ())?;
            readers.push(reader);
        }

        Ok(Self { readers })
    }

    pub fn merged_iter(self, schema: &Schema) -> impl Iterator<Item = RecordBatch> + '_ {
        let adapted_readers = self.readers.into_iter().map(move |reader| reader.flatten());

        kmerge_by(adapted_readers, |a: &RecordBatch, b: &RecordBatch| {
            let a: &TimestampMillisecondArray = a
                .column(0)
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .unwrap();

            let b: &TimestampMillisecondArray = b
                .column(0)
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .unwrap();

            a.value(0) < b.value(0)
        })
        .map(|batch| adapt_batch(schema, batch))
    }

    pub fn merged_schema(&self) -> Schema {
        Schema::try_merge(
            self.readers
                .iter()
                .map(|reader| reader.schema().as_ref().clone()),
        )
        .unwrap()
    }
}