use std::collections::HashMap;

use crate::ids::{SymbolId, next_index, position};

const MEASUREMENT_PREFIX: &str = "#m";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Variable,
    Constant,
    Function { arity: usize },
    Measurement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinConstant {
    Pi,
    E,
    ImaginaryUnit,
    Infinity,
}

impl BuiltinConstant {
    pub const ALL: [BuiltinConstant; 4] = [
        BuiltinConstant::Pi,
        BuiltinConstant::E,
        BuiltinConstant::ImaginaryUnit,
        BuiltinConstant::Infinity,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BuiltinConstant::Pi => "pi",
            BuiltinConstant::E => "e",
            BuiltinConstant::ImaginaryUnit => "i",
            BuiltinConstant::Infinity => "inf",
        }
    }

    pub fn symbol(self) -> SymbolId {
        match self {
            BuiltinConstant::Pi => SymbolId(0),
            BuiltinConstant::E => SymbolId(1),
            BuiltinConstant::ImaginaryUnit => SymbolId(2),
            BuiltinConstant::Infinity => SymbolId(3),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolError {
    KindConflict {
        symbol: SymbolId,
        existing: SymbolKind,
    },
    UnknownSymbolId(SymbolId),
    TableFull,
}

struct SymbolEntry {
    name: String,
    kind: SymbolKind,
}

pub(crate) struct SymbolTable {
    entries: Vec<SymbolEntry>,
    by_name: HashMap<String, SymbolId>,
    entry_limit: u32,
    measurements: u32,
}

impl SymbolTable {
    pub(crate) fn new(entry_limit: u32) -> Self {
        let mut table = Self {
            entries: Vec::new(),
            by_name: HashMap::new(),
            entry_limit,
            measurements: 0,
        };
        for constant in BuiltinConstant::ALL {
            table.entries.push(SymbolEntry {
                name: constant.name().to_string(),
                kind: SymbolKind::Constant,
            });
            table
                .by_name
                .insert(constant.name().to_string(), constant.symbol());
        }
        table
    }

    pub(crate) fn intern(&mut self, name: &str, kind: SymbolKind) -> Result<SymbolId, SymbolError> {
        if let Some(existing) = self.by_name.get(name).copied() {
            let existing_kind = self.kind(existing)?;
            return if existing_kind == kind {
                Ok(existing)
            } else {
                Err(SymbolError::KindConflict {
                    symbol: existing,
                    existing: existing_kind,
                })
            };
        }
        let index =
            next_index(self.entries.len(), self.entry_limit).ok_or(SymbolError::TableFull)?;
        let symbol = SymbolId(index);
        self.entries.push(SymbolEntry {
            name: name.to_string(),
            kind,
        });
        self.by_name.insert(name.to_string(), symbol);
        Ok(symbol)
    }

    pub(crate) fn fresh_measurement(&mut self) -> Result<SymbolId, SymbolError> {
        let mut index = self.measurements + 1;
        loop {
            let name = format!("{MEASUREMENT_PREFIX}{index}");
            if self.by_name.contains_key(&name) {
                index += 1;
                continue;
            }
            self.measurements = index;
            return self.intern(&name, SymbolKind::Measurement);
        }
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<SymbolId> {
        self.by_name.get(name).copied()
    }

    fn entry(&self, symbol: SymbolId) -> Result<&SymbolEntry, SymbolError> {
        self.entries
            .get(position(symbol.0))
            .ok_or(SymbolError::UnknownSymbolId(symbol))
    }

    pub(crate) fn name(&self, symbol: SymbolId) -> Result<&str, SymbolError> {
        self.entry(symbol).map(|entry| entry.name.as_str())
    }

    pub(crate) fn kind(&self, symbol: SymbolId) -> Result<SymbolKind, SymbolError> {
        self.entry(symbol).map(|entry| entry.kind)
    }
}
