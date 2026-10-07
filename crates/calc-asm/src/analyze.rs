use std::collections::{BTreeMap, BTreeSet};

use crate::linear::{Bounds, Condition, Decision, Linear, Poly, Relation};
use crate::parse::{Function, Instruction, Memory, Operand, Place, Program};
use crate::registers::{CALLER_SAVED, Width, is_general, name_at, register};
use crate::semantics::{Control, accesses, control, has_repeat_prefix, writes_destination};

pub const PATH_LIMIT: usize = 4096;
const RESIDUE_LIMIT: i128 = 16;
const VALUES_ANSWERED_ONE_BY_ONE: i128 = 16;
pub const CALL_DEPTH_LIMIT: usize = 32;
const ENDS_THE_PROGRAM: [&str; 8] = [
    "abort",
    "exit",
    "_exit",
    "_Exit",
    "quick_exit",
    "__stack_chk_fail",
    "__assert_fail",
    "__fortify_fail",
];
const LEAVES_WITHOUT_RETURNING: [&str; 7] = [
    "__cxa_throw",
    "__cxa_rethrow",
    "_Unwind_Resume",
    "pthread_exit",
    "longjmp",
    "siglongjmp",
    "__longjmp_chk",
];
const COUNTER_PREFIX: &str = "$";
const SLOT_WIDTH: i128 = 8;
const STACK_STEP: i128 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopProblem {
    SecondExit,
    UnconditionalLatch,
    NoCounter,
    Step,
    Unsigned,
    DependsOnData,
    DependsOnOuterCounter,
    NeverEnds,
    Wraps,
    ChangesStack,
    Overlapping,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    IndirectJump(Place),
    JumpOutside(Place, String),
    Recursion(Place, String),
    RepeatPrefix(Place),
    UnsupportedControl(Place, String),
    RunsPastEnd,
    TooManyPaths,
    CallsTooDeep(Place, String),
    Trap(Place, String),
    LeavesWithoutReturning(Place, String),
    DataRange,
    Loop(Place, LoopProblem),
    InCallee(Place, String, Box<Refusal>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Linear(Linear),
    Low32(Linear),
    Stack(i128),
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Flags {
    Compare(Value, Value, u32),
    ResultWithoutComparedCarry(Value),
    Residue(Linear, i128),
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    names: std::rc::Rc<BTreeMap<String, String>>,
    registers: BTreeMap<String, Value>,
    slots: BTreeMap<i128, (i128, Value)>,
    flags: Flags,
    zero_extended: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Range {
    low: Poly,
    high: Poly,
}

impl Range {
    fn exact(value: Poly) -> Self {
        Self {
            low: value.clone(),
            high: value,
        }
    }

    fn zero() -> Self {
        Self::exact(Poly::zero())
    }

    fn plus(&self, other: &Self) -> Self {
        Self {
            low: self.low.plus(&other.low),
            high: self.high.plus(&other.high),
        }
    }

    fn times(&self, factor: &Poly) -> Self {
        Self {
            low: self.low.times(factor),
            high: self.high.times(factor),
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.low.contains(name) || self.high.contains(name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Tally {
    instructions: Range,
    reads: Range,
    writes: Range,
    memory_known: bool,
}

impl Tally {
    fn zero() -> Self {
        Self {
            instructions: Range::zero(),
            reads: Range::zero(),
            writes: Range::zero(),
            memory_known: true,
        }
    }

    fn plus(&self, other: &Self) -> Self {
        Self {
            instructions: self.instructions.plus(&other.instructions),
            reads: self.reads.plus(&other.reads),
            writes: self.writes.plus(&other.writes),
            memory_known: self.memory_known && other.memory_known,
        }
    }

    fn times(&self, factor: &Poly) -> Self {
        Self {
            instructions: self.instructions.times(factor),
            reads: self.reads.times(factor),
            writes: self.writes.times(factor),
            memory_known: self.memory_known,
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.instructions.contains(name) || self.reads.contains(name) || self.writes.contains(name)
    }
}

#[derive(Clone, Debug)]
struct Path {
    state: State,
    bounds: Bounds,
    conditions: BTreeSet<Condition>,
    tally: Tally,
    outside: BTreeSet<String>,
    computed_writes: bool,
    resume: Option<usize>,
    data_forked: bool,
}

#[derive(Clone, Debug)]
enum Outcome {
    Ended(Path),
    Refused(Path, Refusal),
}

#[derive(Clone, Copy, Debug)]
struct Region {
    start: usize,
    latch: usize,
}

#[derive(Clone, Debug)]
struct LoopShape {
    test: usize,
    continues_when_taken: bool,
    pass_start: usize,
    exit_to: usize,
    side_exits: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Return,
    At { jump: usize, leave: usize },
}

const NOWHERE: usize = usize::MAX;

struct Walker<'a> {
    program: &'a Program,
    function: &'a Function,
    regions: Vec<Region>,
    call_stack: Vec<String>,
    symbolic_forks: bool,
}

enum Resolved {
    Local(usize),
    Function(usize),
    Outside(String),
    Indirect,
}

enum Predicate {
    Condition(Condition),
    Data,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Count {
    pub low: String,
    pub high: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counts {
    pub instructions: Count,
    pub reads: Option<Count>,
    pub writes: Option<Count>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub conditions: Vec<String>,
    pub outcome: Result<Counts, Refusal>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionReport {
    pub name: String,
    pub outcome: Result<Vec<Case>, Refusal>,
    pub inputs: Vec<String>,
    pub outside: Vec<String>,
    pub assumes_separate_stack: bool,
    pub has_loops: bool,
}

fn is_counter(name: &str) -> bool {
    name.starts_with(COUNTER_PREFIX)
}

fn zero_extended_read(state: &State, instruction: &Instruction) -> Option<(String, Linear)> {
    let mut names: Vec<&String> = Vec::new();
    for (position, operand) in instruction.operands.iter().enumerate() {
        match operand {
            Operand::Register(name)
                if !(position == 0 && only_writes_its_destination(&instruction.mnemonic)) =>
            {
                names.push(name);
            }
            Operand::Memory(memory) => names.extend(memory.base.iter().chain(memory.index.iter())),
            _ => {}
        }
    }
    names.into_iter().find_map(|name| {
        let found = register(name)?;
        if found.width != Width::Full || !state.zero_extended.contains(&found.family) {
            return None;
        }
        match state.registers.get(&found.family) {
            Some(Value::Linear(linear))
                if !linear.terms().keys().any(|symbol| is_counter(symbol)) =>
            {
                Some((found.family, linear.clone()))
            }
            _ => None,
        }
    })
}

fn zero_extension_forks(path: &Path, instruction: &Instruction) -> Option<Vec<Path>> {
    let (family, value) = zero_extended_read(&path.state, instruction)?;
    Some(settled_zero_extension(path, &family, &value))
}

fn settled_zero_extension(path: &Path, family: &str, value: &Linear) -> Vec<Path> {
    let settled = |mut path: Path, nonnegative: bool| {
        let widened = if nonnegative {
            value.clone()
        } else {
            value.plus_constant(1_i128 << 32)
        };
        path.state
            .registers
            .insert(family.to_owned(), Value::Linear(widened));
        path.state.zero_extended.remove(family);
        path
    };
    let condition = Condition::at_least_zero(value.clone());
    match path.bounds.decide(&condition) {
        Decision::Holds => vec![settled(path.clone(), true)],
        Decision::Fails => vec![settled(path.clone(), false)],
        Decision::Open => {
            let mut forks = Vec::new();
            for (condition, nonnegative) in [(condition.negated(), false), (condition, true)] {
                let mut fork = path.clone();
                if let Some(bounds) = fork.bounds.with(&condition) {
                    fork.bounds = bounds;
                    fork.conditions.insert(condition);
                    forks.push(settled(fork, nonnegative));
                }
            }
            forks
        }
    }
}

fn low_halves(linear: &Linear) -> Linear {
    let renamed: BTreeMap<String, Linear> = linear
        .terms()
        .keys()
        .filter_map(|symbol| {
            let found = register(symbol)?;
            (found.width == Width::Full && is_general(&found.family)).then(|| {
                (
                    symbol.clone(),
                    Linear::symbol(&name_at(&found.family, Width::Double)),
                )
            })
        })
        .collect();
    if renamed.is_empty() {
        return linear.clone();
    }
    linear
        .substituted(&renamed)
        .unwrap_or_else(|| linear.clone())
}

fn int_ranges(function: &Function) -> Bounds {
    let mut bounds = Bounds::default();
    for (family, name) in entry_names(function) {
        if name != name_at(&family, Width::Double) {
            continue;
        }
        let symbol = Linear::symbol(&name);
        for condition in [
            Condition::at_least_zero(symbol.plus_constant(1_i128 << 31)),
            Condition::at_least_zero(symbol.scaled(-1).plus_constant((1_i128 << 31) - 1)),
        ] {
            if let Some(narrowed) = bounds.with(&condition) {
                bounds = narrowed;
            }
        }
    }
    bounds
}

fn only_writes_its_destination(mnemonic: &str) -> bool {
    mnemonic.starts_with("mov") || mnemonic.starts_with("set") || matches!(mnemonic, "lea" | "pop")
}

fn entry_names(function: &Function) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let mut written: BTreeSet<String> = BTreeSet::new();
    for instruction in &function.instructions {
        let mut reads: Vec<&String> = Vec::new();
        let mut destination: Option<&String> = None;
        for (position, operand) in instruction.operands.iter().enumerate() {
            match operand {
                Operand::Register(name)
                    if position == 0 && only_writes_its_destination(&instruction.mnemonic) =>
                {
                    destination = Some(name);
                }
                Operand::Register(name) => {
                    if position == 0 {
                        destination = Some(name);
                    }
                    reads.push(name);
                }
                Operand::Memory(memory) => {
                    reads.extend(memory.base.iter().chain(memory.index.iter()))
                }
                _ => {}
            }
        }
        for name in reads {
            if let Some(found) = register(name)
                && is_general(&found.family)
                && !written.contains(&found.family)
            {
                let width = if found.width == Width::Double {
                    Width::Double
                } else {
                    Width::Full
                };
                let entry = names
                    .entry(found.family.clone())
                    .or_insert_with(|| name_at(&found.family, width));
                if width == Width::Double {
                    *entry = name_at(&found.family, Width::Double);
                }
            }
        }
        if let Some(found) = destination.and_then(|name| register(name)) {
            written.insert(found.family);
        }
    }
    names
}

fn entry_symbol(state: &State, family: &str) -> String {
    state
        .names
        .get(family)
        .cloned()
        .unwrap_or_else(|| name_at(family, Width::Full))
}

fn entry_state(function: &Function, given: &BTreeMap<String, i128>) -> State {
    let names = entry_names(function);
    let mut registers = BTreeMap::from([("rsp".to_owned(), Value::Stack(0))]);
    for (name, value) in given {
        if let Some(found) = register(name) {
            let written = match found.width {
                Width::Double => unsigned_at(*value, 32),
                _ => *value,
            };
            registers.insert(found.family, Value::Linear(Linear::constant(written)));
        }
    }
    State {
        names: std::rc::Rc::new(names),
        registers,
        slots: BTreeMap::new(),
        flags: Flags::Unknown,
        zero_extended: BTreeSet::new(),
    }
}

fn find_regions(function: &Function) -> Vec<Region> {
    let mut regions: Vec<Region> = Vec::new();
    for (index, instruction) in function.instructions.iter().enumerate() {
        if !matches!(
            control(instruction),
            Control::Jump | Control::ConditionalJump
        ) {
            continue;
        }
        let target = instruction
            .operands
            .first()
            .and_then(|operand| local_target(function, operand));
        if let Some(target) = target.filter(|target| *target <= index)
            && reaches(function, target, index)
        {
            regions.push(Region {
                start: target,
                latch: index,
            });
        }
    }
    regions.sort_by_key(|region| std::cmp::Reverse(region.latch - region.start));
    regions
}

fn successors(function: &Function, index: usize) -> Vec<usize> {
    let Some(instruction) = function.instructions.get(index) else {
        return Vec::new();
    };
    let target = || {
        instruction
            .operands
            .first()
            .and_then(|operand| local_target(function, operand))
    };
    match control(instruction) {
        Control::Return | Control::Unsupported => Vec::new(),
        Control::Jump => target().into_iter().collect(),
        Control::ConditionalJump => std::iter::once(index + 1).chain(target()).collect(),
        Control::SystemCall if matches!(instruction.mnemonic.as_str(), "ud2" | "hlt") => Vec::new(),
        Control::Plain | Control::Call | Control::SystemCall => vec![index + 1],
    }
}

fn reaches(function: &Function, from: usize, to: usize) -> bool {
    let mut seen = BTreeSet::new();
    let mut pending = vec![from];
    while let Some(index) = pending.pop() {
        if index == to {
            return true;
        }
        if index >= function.instructions.len() || !seen.insert(index) {
            continue;
        }
        pending.extend(successors(function, index));
    }
    false
}

fn local_target(function: &Function, operand: &Operand) -> Option<usize> {
    match operand {
        Operand::Target(name) => function.labels.get(name).copied(),
        Operand::Address { label, .. } => function.labels.get(label).copied(),
        _ => None,
    }
}

impl<'a> Walker<'a> {
    fn new(program: &'a Program, function: &'a Function, call_stack: Vec<String>) -> Self {
        Self {
            program,
            function,
            regions: find_regions(function),
            call_stack,
            symbolic_forks: true,
        }
    }

    fn resolve(&self, operand: &Operand) -> Resolved {
        match self.resolve_target(operand) {
            Resolved::Function(callee) => {
                let name = &self.program.functions[callee].name;
                match name.strip_suffix("@plt") {
                    Some(library) => Resolved::Outside(library.to_owned()),
                    None => Resolved::Function(callee),
                }
            }
            other => other,
        }
    }

    fn resolve_target(&self, operand: &Operand) -> Resolved {
        if let Some(index) = local_target(self.function, operand) {
            return Resolved::Local(index);
        }
        match operand {
            Operand::Target(name) => self
                .program
                .functions
                .iter()
                .position(|function| {
                    function.name == *name || function.labels.get(name) == Some(&0)
                })
                .map_or_else(|| Resolved::Outside(name.clone()), Resolved::Function),
            Operand::Address { label, name } => self
                .program
                .functions
                .iter()
                .position(|function| {
                    function.instructions.first().is_some_and(|first| {
                        matches!(first.place, Place::Address(address) if format!("0x{address:x}") == *label)
                    })
                })
                .map_or_else(|| Resolved::Outside(name.clone()), Resolved::Function),
            Operand::Register(_) | Operand::Memory(_) | Operand::Immediate(_) => Resolved::Indirect,
        }
    }

    fn walk(
        &self,
        from: usize,
        path: Path,
        active: &[usize],
        stop: Stop,
    ) -> Result<Vec<Outcome>, Refusal> {
        let mut work = vec![(from, path)];
        let mut finished = Vec::new();
        while let Some((start, mut path)) = work.pop() {
            let mut index = start;
            loop {
                if finished.len() + work.len() > PATH_LIMIT {
                    return Err(Refusal::TooManyPaths);
                }
                if let Stop::At { leave, .. } = stop
                    && index == leave
                {
                    path.resume = Some(index);
                    finished.push(Outcome::Ended(path));
                    break;
                }
                if let Some(region) = self.entering(index, active) {
                    let mut entered = self.loop_entry_forks(region, path);
                    let Some(entering) = entered.pop() else {
                        break;
                    };
                    for fork in entered {
                        work.push((index, fork));
                    }
                    let mut outcomes = self.summarize(region, index, entering, active)?;
                    let Some((first, first_next)) = outcomes.pop() else {
                        break;
                    };
                    for (outcome, next_index) in outcomes {
                        match outcome {
                            Outcome::Ended(next) => work.push((next_index, next)),
                            refused @ Outcome::Refused(..) => finished.push(refused),
                        }
                    }
                    match first {
                        Outcome::Ended(next) => {
                            path = next;
                            index = first_next;
                            continue;
                        }
                        refused @ Outcome::Refused(..) => {
                            finished.push(refused);
                            break;
                        }
                    }
                }
                let Some(instruction) = self.function.instructions.get(index) else {
                    return Err(Refusal::RunsPastEnd);
                };
                if has_repeat_prefix(instruction) {
                    return Err(Refusal::RepeatPrefix(instruction.place));
                }
                while let Some(mut forks) = zero_extension_forks(&path, instruction) {
                    let Some(first) = forks.pop() else {
                        break;
                    };
                    for fork in forks {
                        work.push((index, fork));
                    }
                    path = first;
                }
                count(&mut path, instruction);
                match control(instruction) {
                    Control::Return => {
                        finished.push(Outcome::Ended(path));
                        break;
                    }
                    Control::Jump => {
                        let operand = instruction.operands.first().cloned();
                        match operand.map(|operand| self.resolve(&operand)) {
                            Some(Resolved::Local(target))
                                if target > index
                                    || !self.is_latch(index)
                                    || active
                                        .iter()
                                        .any(|region| self.regions[*region].latch == index) =>
                            {
                                index = target;
                            }
                            Some(Resolved::Local(_)) => {
                                return Err(Refusal::Loop(
                                    instruction.place,
                                    LoopProblem::Overlapping,
                                ));
                            }
                            Some(Resolved::Function(callee)) => {
                                for outcome in self.call(callee, instruction.place, path)? {
                                    finished.push(outcome);
                                }
                                break;
                            }
                            Some(Resolved::Outside(name)) => {
                                return Err(Refusal::JumpOutside(instruction.place, name));
                            }
                            Some(Resolved::Indirect) | None => {
                                return Err(Refusal::IndirectJump(instruction.place));
                            }
                        }
                    }
                    Control::ConditionalJump => {
                        if let Stop::At { jump, .. } = stop
                            && index == jump
                        {
                            path.resume = Some(index);
                            finished.push(Outcome::Ended(path));
                            break;
                        }
                        let resolved = instruction
                            .operands
                            .first()
                            .map(|operand| self.resolve(operand));
                        if let Some(Resolved::Function(callee)) = resolved {
                            let predicate =
                                predicate(&path.state.flags, &instruction.mnemonic, &path.bounds);
                            let decision = match &predicate {
                                Predicate::Condition(condition) => path.bounds.decide(condition),
                                Predicate::Data => Decision::Open,
                            };
                            match (decision, predicate) {
                                (Decision::Holds, _) => {
                                    finished.extend(self.call(callee, instruction.place, path)?);
                                    break;
                                }
                                (Decision::Fails, _) => index += 1,
                                (Decision::Open, Predicate::Condition(condition)) => {
                                    let mut taken = path.clone();
                                    if let Some(bounds) = taken.bounds.with(&condition) {
                                        taken.bounds = bounds;
                                        taken.conditions.insert(condition.clone());
                                        finished.extend(self.call(
                                            callee,
                                            instruction.place,
                                            taken,
                                        )?);
                                    }
                                    let negated = condition.negated();
                                    match path.bounds.with(&negated) {
                                        Some(bounds) => {
                                            path.bounds = bounds;
                                            path.conditions.insert(negated);
                                            index += 1;
                                        }
                                        None => break,
                                    }
                                }
                                (Decision::Open, Predicate::Data) => {
                                    path.data_forked = true;
                                    finished.extend(self.call(
                                        callee,
                                        instruction.place,
                                        path.clone(),
                                    )?);
                                    index += 1;
                                }
                            }
                            continue;
                        }
                        let target = match resolved {
                            Some(Resolved::Local(target))
                                if target > index || !self.is_latch(index) =>
                            {
                                target
                            }
                            Some(Resolved::Local(_)) => {
                                return Err(Refusal::Loop(
                                    instruction.place,
                                    LoopProblem::Overlapping,
                                ));
                            }
                            Some(Resolved::Outside(name)) => {
                                return Err(Refusal::JumpOutside(instruction.place, name));
                            }
                            _ => return Err(Refusal::IndirectJump(instruction.place)),
                        };
                        match predicate(&path.state.flags, &instruction.mnemonic, &path.bounds) {
                            Predicate::Condition(condition)
                                if self.symbolic_forks
                                    || condition_is_constant(&condition)
                                    || path.bounds.decide(&condition) != Decision::Open =>
                            {
                                match path.bounds.decide(&condition) {
                                    Decision::Holds => index = target,
                                    Decision::Fails => index += 1,
                                    Decision::Open => {
                                        let mut taken = path.clone();
                                        if let Some(bounds) = taken.bounds.with(&condition) {
                                            taken.bounds = bounds;
                                            taken.conditions.insert(condition.clone());
                                            work.push((target, taken));
                                        }
                                        let negated = condition.negated();
                                        match path.bounds.with(&negated) {
                                            Some(bounds) => {
                                                path.bounds = bounds;
                                                path.conditions.insert(negated);
                                                index += 1;
                                            }
                                            None => break,
                                        }
                                    }
                                }
                            }
                            Predicate::Condition(_) | Predicate::Data => {
                                path.data_forked = true;
                                work.push((target, path.clone()));
                                index += 1;
                            }
                        }
                    }
                    Control::Call => {
                        let operand = instruction.operands.first().cloned();
                        match operand.map(|operand| self.resolve(&operand)) {
                            Some(Resolved::Function(callee)) => {
                                let mut outcomes = self.call(callee, instruction.place, path)?;
                                let Some(first) = outcomes.pop() else {
                                    break;
                                };
                                for outcome in outcomes {
                                    match outcome {
                                        Outcome::Ended(next) => work.push((index + 1, next)),
                                        refused @ Outcome::Refused(..) => finished.push(refused),
                                    }
                                }
                                match first {
                                    Outcome::Ended(next) => path = next,
                                    refused @ Outcome::Refused(..) => {
                                        finished.push(refused);
                                        break;
                                    }
                                }
                            }
                            Some(Resolved::Outside(name))
                                if ENDS_THE_PROGRAM.contains(&name.as_str()) =>
                            {
                                finished.push(Outcome::Refused(
                                    path,
                                    Refusal::Trap(instruction.place, name),
                                ));
                                break;
                            }
                            Some(Resolved::Outside(name))
                                if LEAVES_WITHOUT_RETURNING.contains(&name.as_str()) =>
                            {
                                finished.push(Outcome::Refused(
                                    path,
                                    Refusal::LeavesWithoutReturning(instruction.place, name),
                                ));
                                break;
                            }
                            Some(Resolved::Outside(name)) => {
                                path.outside.insert(name);
                                clobber(&mut path.state);
                            }
                            _ => {
                                path.outside.insert(String::new());
                                clobber(&mut path.state);
                            }
                        }
                        index += 1;
                    }
                    Control::SystemCall
                        if matches!(instruction.mnemonic.as_str(), "ud2" | "hlt") =>
                    {
                        let trap = Refusal::Trap(instruction.place, instruction.mnemonic.clone());
                        finished.push(Outcome::Refused(path, trap));
                        break;
                    }
                    Control::SystemCall => {
                        path.outside.insert(instruction.mnemonic.clone());
                        clobber(&mut path.state);
                        index += 1;
                    }
                    Control::Unsupported => {
                        return Err(Refusal::UnsupportedControl(
                            instruction.place,
                            instruction.mnemonic.clone(),
                        ));
                    }
                    Control::Plain => {
                        if let Some(mut forks) = split_residue(&path, instruction) {
                            let Some(first) = forks.pop() else {
                                break;
                            };
                            for fork in forks {
                                work.push((index + 1, fork));
                            }
                            path = first;
                        } else if step(&mut path.state, instruction) {
                            path.computed_writes = true;
                        }
                        index += 1;
                    }
                }
            }
        }
        Ok(finished)
    }

    fn is_latch(&self, index: usize) -> bool {
        self.regions.iter().any(|region| region.latch == index)
    }

    fn loop_entry_forks(&self, region: usize, path: Path) -> Vec<Path> {
        let Region { start, latch } = self.regions[region];
        let body = self
            .function
            .instructions
            .get(start..=latch)
            .unwrap_or_default();
        let mut paths = vec![path];
        for instruction in body {
            paths = paths
                .into_iter()
                .flat_map(|path| match zero_extended_read(&path.state, instruction) {
                    Some((family, value)) => settled_zero_extension(&path, &family, &value),
                    None => vec![path],
                })
                .collect();
        }
        paths
    }

    fn entering(&self, index: usize, active: &[usize]) -> Option<usize> {
        self.regions
            .iter()
            .enumerate()
            .find(|(position, region)| {
                !active.contains(position) && region.start <= index && index <= region.latch
            })
            .map(|(position, _)| position)
    }

    fn call(&self, callee: usize, place: Place, path: Path) -> Result<Vec<Outcome>, Refusal> {
        let function = &self.program.functions[callee];
        if self.call_stack.contains(&function.name) {
            return Err(Refusal::Recursion(place, function.name.clone()));
        }
        if self.call_stack.len() > CALL_DEPTH_LIMIT {
            return Err(Refusal::CallsTooDeep(place, function.name.clone()));
        }
        let mut stack = self.call_stack.clone();
        stack.push(function.name.clone());
        let walker = Walker::new(self.program, function, stack);
        let caller_state = path.state.clone();
        let frame_escapes = caller_state
            .registers
            .iter()
            .any(|(family, value)| family != "rsp" && matches!(value, Value::Stack(_)));
        let mut entry = path;
        for (family, value) in entry.state.registers.iter_mut() {
            if family != "rsp" && matches!(value, Value::Stack(_)) {
                *value = Value::Unknown;
            }
        }
        for family in crate::registers::GENERAL_FAMILIES {
            if !entry.state.registers.contains_key(family) {
                let symbol = entry_symbol(&caller_state, family);
                entry
                    .state
                    .registers
                    .insert(family.to_owned(), Value::Linear(Linear::symbol(&symbol)));
            }
        }
        entry.state.names = std::rc::Rc::new(entry_names(function));
        entry
            .state
            .registers
            .insert("rsp".to_owned(), Value::Stack(0));
        entry.state.slots = BTreeMap::new();
        entry.state.flags = Flags::Unknown;
        let outcomes = walker
            .walk(0, entry, &[], Stop::Return)
            .map_err(|refusal| {
                Refusal::InCallee(place, function.name.clone(), Box::new(refusal))
            })?;
        Ok(outcomes
            .into_iter()
            .map(|outcome| match outcome {
                Outcome::Ended(mut returned) => {
                    let mut state = caller_state.clone();
                    clobber(&mut state);
                    if frame_escapes {
                        for (_, value) in state.slots.values_mut() {
                            *value = Value::Unknown;
                        }
                    }
                    returned.state = state;
                    Outcome::Ended(returned)
                }
                refused @ Outcome::Refused(..) => refused,
            })
            .collect())
    }

    fn check_region(&self, region: Region) -> Result<LoopShape, Refusal> {
        let latch = &self.function.instructions[region.latch];
        let mut exits: Vec<(usize, usize)> = Vec::new();
        for index in region.start..=region.latch {
            let instruction = &self.function.instructions[index];
            match control(instruction) {
                Control::Return => {
                    return Err(Refusal::Loop(instruction.place, LoopProblem::SecondExit));
                }
                Control::Jump | Control::ConditionalJump => {
                    let target = instruction
                        .operands
                        .first()
                        .and_then(|operand| local_target(self.function, operand));
                    match target {
                        Some(target) if region.start <= target && target <= region.latch => {}
                        Some(target) if control(instruction) == Control::ConditionalJump => {
                            exits.push((index, target));
                        }
                        _ => {
                            return Err(Refusal::Loop(instruction.place, LoopProblem::SecondExit));
                        }
                    }
                }
                _ => {}
            }
        }
        let shape = if control(latch) == Control::Jump {
            match exits.as_slice() {
                [(test, exit_to)] if *test < region.latch => LoopShape {
                    test: *test,
                    continues_when_taken: false,
                    pass_start: test + 1,
                    exit_to: *exit_to,
                    side_exits: Vec::new(),
                },
                [] => return Err(Refusal::Loop(latch.place, LoopProblem::UnconditionalLatch)),
                [(test, _), ..] => {
                    let place = self.function.instructions[*test].place;
                    return Err(Refusal::Loop(place, LoopProblem::SecondExit));
                }
            }
        } else {
            if exits.len() > 1 {
                let place = self.function.instructions[exits[1].0].place;
                return Err(Refusal::Loop(place, LoopProblem::SecondExit));
            }
            LoopShape {
                test: region.latch,
                continues_when_taken: true,
                pass_start: region.start,
                exit_to: region.latch + 1,
                side_exits: exits,
            }
        };
        let overlapping = self.regions.iter().any(|other| {
            (other.start < region.start
                && region.start <= other.latch
                && other.latch < region.latch)
                || (region.start < other.start
                    && other.start <= region.latch
                    && region.latch < other.latch)
                || (other.start == region.start && other.latch != region.latch)
        });
        if overlapping {
            return Err(Refusal::Loop(latch.place, LoopProblem::Overlapping));
        }
        Ok(shape)
    }

    fn modified(
        &self,
        region: Region,
        state: &State,
    ) -> Result<(BTreeSet<String>, BTreeSet<i128>), Refusal> {
        let mut registers = BTreeSet::new();
        let mut slots = BTreeSet::new();
        for index in region.start..=region.latch {
            let instruction = &self.function.instructions[index];
            let mnemonic = instruction.mnemonic.as_str();
            if matches!(mnemonic, "push" | "pop" | "leave" | "enter") {
                return Err(Refusal::Loop(instruction.place, LoopProblem::ChangesStack));
            }
            if control(instruction) == Control::Call || control(instruction) == Control::SystemCall
            {
                registers.extend(CALLER_SAVED.iter().map(|name| (*name).to_owned()));
                continue;
            }
            if matches!(
                mnemonic,
                "mul" | "div" | "idiv" | "cdq" | "cqo" | "cpuid" | "rdtsc"
            ) {
                registers.insert("rax".to_owned());
                registers.insert("rdx".to_owned());
            }
            if is_register_nop(instruction) {
                continue;
            }
            if (mnemonic == "xchg" || mnemonic == "xadd")
                && let Some(Operand::Register(name)) = instruction.operands.get(1)
            {
                registers.extend(register(name).map(|found| found.family));
            }
            if !writes_destination(mnemonic) {
                continue;
            }
            match instruction.operands.first() {
                Some(Operand::Register(name)) => {
                    if let Some(found) = register(name) {
                        if found.family == "rsp" || found.family == "rbp" {
                            return Err(Refusal::Loop(
                                instruction.place,
                                LoopProblem::ChangesStack,
                            ));
                        }
                        registers.insert(found.family);
                    }
                }
                Some(Operand::Memory(memory)) => {
                    if let Some(Value::Stack(offset)) = address(state, memory) {
                        slots.insert(offset);
                    }
                }
                _ => {}
            }
        }
        Ok((registers, slots))
    }

    fn summarize(
        &self,
        region_index: usize,
        entry: usize,
        path: Path,
        active: &[usize],
    ) -> Result<Vec<(Outcome, usize)>, Refusal> {
        let region = self.regions[region_index];
        let loop_shape = self.check_region(region)?;
        if let Some((side, side_target)) = loop_shape.side_exits.first().copied() {
            return self.split_side_exit(
                region_index,
                entry,
                path,
                active,
                &loop_shape,
                side,
                side_target,
            );
        }
        self.summarize_shaped(region_index, entry, path, active, &loop_shape)
    }

    fn summarize_shaped(
        &self,
        region_index: usize,
        entry: usize,
        path: Path,
        active: &[usize],
        loop_shape: &LoopShape,
    ) -> Result<Vec<(Outcome, usize)>, Refusal> {
        let region = self.regions[region_index];
        let latch_place = self.function.instructions[loop_shape.test].place;
        let refuse = |problem| Refusal::Loop(latch_place, problem);
        let (registers, slots) = self.modified(region, &path.state)?;
        let mut general = path.state.clone();
        let mut symbols: BTreeMap<String, Location> = BTreeMap::new();
        let linked = if entry == loop_shape.pass_start {
            self.linked_counters(region, &registers, &path.state)
        } else {
            BTreeMap::new()
        };
        for family in &registers {
            let symbol = format!("{COUNTER_PREFIX}{family}");
            if let Some((representative, difference)) = linked.get(family) {
                let value =
                    Linear::symbol(&format!("{COUNTER_PREFIX}{representative}")).plus(difference);
                general
                    .registers
                    .insert(family.clone(), Value::Linear(value));
                continue;
            }
            general
                .registers
                .insert(family.clone(), Value::Linear(Linear::symbol(&symbol)));
            let width = self.counter_width(region, family);
            symbols.insert(symbol, Location::Register(family.clone(), width));
        }
        for offset in &slots {
            let symbol = format!("{COUNTER_PREFIX}@{offset}");
            let width = general
                .slots
                .get(offset)
                .map_or(SLOT_WIDTH, |(width, _)| *width);
            general
                .slots
                .insert(*offset, (width, Value::Linear(Linear::symbol(&symbol))));
            symbols.insert(symbol, Location::Slot(*offset));
        }
        general.flags = Flags::Unknown;
        let mut inner_active = active.to_vec();
        inner_active.push(region_index);
        let pass = |from: usize| -> Result<Vec<(BTreeSet<Condition>, PassShape)>, Refusal> {
            let start = Path {
                state: general.clone(),
                bounds: path.bounds.clone(),
                conditions: BTreeSet::new(),
                tally: Tally::zero(),
                outside: BTreeSet::new(),
                computed_writes: false,
                resume: None,
                data_forked: false,
            };
            let mut inner = Walker::new(self.program, self.function, self.call_stack.clone());
            inner.symbolic_forks = false;
            let outcomes = inner.walk(
                from,
                start,
                &inner_active,
                Stop::At {
                    jump: loop_shape.test,
                    leave: NOWHERE,
                },
            )?;
            if outcomes.iter().any(|outcome| {
                matches!(outcome, Outcome::Ended(end) if end.resume != Some(loop_shape.test))
            }) {
                return Err(refuse(LoopProblem::SecondExit));
            }
            let mut groups: Vec<(BTreeSet<Condition>, Vec<Outcome>)> = Vec::new();
            for outcome in outcomes {
                let conditions = match &outcome {
                    Outcome::Ended(end) | Outcome::Refused(end, _) => end.conditions.clone(),
                };
                if conditions.iter().any(|condition| {
                    condition
                        .expression
                        .terms()
                        .keys()
                        .any(|name| is_counter(name))
                }) {
                    return Err(refuse(LoopProblem::DependsOnOuterCounter));
                }
                match groups
                    .iter_mut()
                    .find(|(existing, _)| *existing == conditions)
                {
                    Some((_, members)) => members.push(outcome),
                    None => groups.push((conditions, vec![outcome])),
                }
            }
            groups
                .into_iter()
                .map(|(conditions, members)| {
                    let bounds = conditions
                        .iter()
                        .try_fold(path.bounds.clone(), |bounds, condition| {
                            bounds.with(condition)
                        })
                        .unwrap_or_else(|| path.bounds.clone());
                    shape(
                        members,
                        &symbols,
                        &self.function.instructions[loop_shape.test],
                        loop_shape.continues_when_taken,
                        &bounds,
                    )
                    .map(|shape| (conditions, shape))
                    .map_err(refuse)
                })
                .collect()
        };
        let partial = if entry == loop_shape.pass_start {
            None
        } else {
            let mut groups = pass(entry)?;
            if groups.len() != 1 {
                return Err(refuse(LoopProblem::DependsOnData));
            }
            groups.pop().map(|(_, shape)| shape)
        };
        let mut outcomes = Vec::new();
        for (conditions, full) in pass(loop_shape.pass_start)? {
            let mut group_path = path.clone();
            let mut feasible = true;
            for condition in conditions {
                match group_path.bounds.with(&condition) {
                    Some(bounds) => {
                        group_path.bounds = bounds;
                        group_path.conditions.insert(condition);
                    }
                    None => feasible = false,
                }
            }
            if feasible {
                outcomes.extend(self.close_loop(
                    group_path,
                    &full,
                    partial.as_ref(),
                    &symbols,
                    &registers,
                    &slots,
                    latch_place,
                )?);
            }
        }
        Ok(outcomes
            .into_iter()
            .map(|outcome| (outcome, loop_shape.exit_to))
            .collect())
    }

    fn counter_width(&self, region: Region, family: &str) -> CounterWidth {
        let mut narrow = false;
        for index in region.start..=region.latch {
            for operand in &self.function.instructions[index].operands {
                let names: Vec<&String> = match operand {
                    Operand::Register(name) => vec![name],
                    Operand::Memory(memory) => {
                        memory.base.iter().chain(memory.index.iter()).collect()
                    }
                    _ => Vec::new(),
                };
                for name in names {
                    match register(name) {
                        Some(found) if found.family == family && found.width == Width::Double => {
                            narrow = true;
                        }
                        Some(found) if found.family == family => return CounterWidth::Full,
                        _ => {}
                    }
                }
            }
        }
        if narrow {
            CounterWidth::Double
        } else {
            CounterWidth::Full
        }
    }

    fn syntactic_steps(
        &self,
        region: Region,
        registers: &BTreeSet<String>,
    ) -> BTreeMap<String, i128> {
        let nested: Vec<Region> = self
            .regions
            .iter()
            .copied()
            .filter(|other| {
                region.start <= other.start
                    && other.latch <= region.latch
                    && (other.start, other.latch) != (region.start, region.latch)
            })
            .collect();
        let mut skipped: BTreeSet<usize> = BTreeSet::new();
        for index in region.start..=region.latch {
            let instruction = &self.function.instructions[index];
            if matches!(
                control(instruction),
                Control::Jump | Control::ConditionalJump
            ) && let Some(target) = instruction
                .operands
                .first()
                .and_then(|operand| local_target(self.function, operand))
                && target > index
                && target <= region.latch
            {
                skipped.extend(index + 1..target);
            }
        }
        let mut steps: BTreeMap<String, Option<i128>> = BTreeMap::new();
        for index in region.start..=region.latch {
            let instruction = &self.function.instructions[index];
            let inside_nested = nested
                .iter()
                .any(|other| other.start <= index && index <= other.latch);
            let mnemonic = instruction.mnemonic.as_str();
            let written: Vec<String> = registers
                .iter()
                .filter(|family| {
                    instruction.operands.first().is_some_and(|operand| {
                        matches!(operand, Operand::Register(name) if register(name).is_some_and(|found| found.family == **family))
                    }) && writes_destination(mnemonic)
                })
                .cloned()
                .collect();
            let clobbers = matches!(control(instruction), Control::Call | Control::SystemCall)
                || matches!(
                    mnemonic,
                    "mul" | "div" | "idiv" | "cdq" | "cqo" | "cpuid" | "rdtsc" | "xchg" | "xadd"
                );
            if clobbers {
                for family in registers {
                    steps.insert(family.clone(), None);
                }
                continue;
            }
            for family in written {
                let amount = match (mnemonic, instruction.operands.get(1)) {
                    ("add", Some(Operand::Immediate(value))) => Some(*value),
                    ("sub", Some(Operand::Immediate(value))) => Some(-value),
                    ("inc", None) => Some(1),
                    ("dec", None) => Some(-1),
                    _ => None,
                };
                let entry = steps.entry(family).or_insert(Some(0));
                *entry = match (*entry, amount) {
                    (Some(total), Some(amount)) if !inside_nested && !skipped.contains(&index) => {
                        Some(total + amount)
                    }
                    _ => None,
                };
            }
        }
        steps
            .into_iter()
            .filter_map(|(family, step)| step.filter(|step| *step != 0).map(|step| (family, step)))
            .collect()
    }

    fn linked_counters(
        &self,
        region: Region,
        registers: &BTreeSet<String>,
        state: &State,
    ) -> BTreeMap<String, (String, Linear)> {
        let steps = self.syntactic_steps(region, registers);
        let mut linked = BTreeMap::new();
        let mut representatives: BTreeMap<i128, (String, Linear)> = BTreeMap::new();
        for (family, step) in steps {
            let width = self.counter_width(region, &family);
            let Value::Linear(initial) =
                read_location(state, &Location::Register(family.clone(), width))
            else {
                continue;
            };
            match representatives.get(&step) {
                Some((representative, representative_initial)) => {
                    linked.insert(
                        family,
                        (
                            representative.clone(),
                            initial.minus(representative_initial),
                        ),
                    );
                }
                None => {
                    representatives.insert(step, (family, initial));
                }
            }
        }
        linked
    }

    #[allow(clippy::too_many_arguments)]
    fn split_side_exit(
        &self,
        region_index: usize,
        entry: usize,
        path: Path,
        active: &[usize],
        loop_shape: &LoopShape,
        side: usize,
        side_target: usize,
    ) -> Result<Vec<(Outcome, usize)>, Refusal> {
        let side_place = self.function.instructions[side].place;
        let refuse = || Refusal::Loop(side_place, LoopProblem::SecondExit);
        let region = self.regions[region_index];
        let (registers, slots) = self.modified(region, &path.state)?;
        let mut general = path.state.clone();
        for family in &registers {
            general.registers.insert(
                family.clone(),
                Value::Linear(Linear::symbol(&format!("{COUNTER_PREFIX}{family}"))),
            );
        }
        for offset in &slots {
            if let Some((_, value)) = general.slots.get_mut(offset) {
                *value = Value::Linear(Linear::symbol(&format!("{COUNTER_PREFIX}@{offset}")));
            }
        }
        general.flags = Flags::Unknown;
        let mut inner_active = active.to_vec();
        inner_active.push(region_index);
        let mut probe = Walker::new(self.program, self.function, self.call_stack.clone());
        probe.symbolic_forks = false;
        let start = Path {
            state: general,
            bounds: path.bounds.clone(),
            conditions: BTreeSet::new(),
            tally: Tally::zero(),
            outside: BTreeSet::new(),
            computed_writes: false,
            resume: None,
            data_forked: false,
        };
        let probes = probe.walk(
            loop_shape.pass_start,
            start,
            &inner_active,
            Stop::At {
                jump: side,
                leave: NOWHERE,
            },
        )?;
        let mnemonic = &self.function.instructions[side].mnemonic;
        let mut taken: Option<Condition> = None;
        for outcome in &probes {
            let Outcome::Ended(end) = outcome else {
                return Err(refuse());
            };
            if end.resume != Some(side) {
                return Err(refuse());
            }
            let Predicate::Condition(condition) =
                predicate(&end.state.flags, mnemonic, &end.bounds)
            else {
                return Err(refuse());
            };
            if condition
                .expression
                .terms()
                .keys()
                .any(|name| is_counter(name))
                || taken.as_ref().is_some_and(|known| *known != condition)
            {
                return Err(refuse());
            }
            taken = Some(condition);
        }
        let taken = taken.ok_or_else(refuse)?;
        let mut results = Vec::new();
        for (branch, holds) in split(&path, &taken) {
            if holds {
                let outcomes = self.walk(
                    entry,
                    branch,
                    &inner_active,
                    Stop::At {
                        jump: side,
                        leave: loop_shape.exit_to,
                    },
                )?;
                for outcome in outcomes {
                    match outcome {
                        Outcome::Ended(mut end) => {
                            let next = match end.resume {
                                Some(index) if index == side => side_target,
                                Some(index) => index,
                                None => return Err(refuse()),
                            };
                            end.resume = None;
                            results.push((Outcome::Ended(end), next));
                        }
                        refused @ Outcome::Refused(..) => results.push((refused, NOWHERE)),
                    }
                }
            } else {
                let mut without = loop_shape.clone();
                without.side_exits.clear();
                results.extend(self.summarize_shaped(
                    region_index,
                    entry,
                    branch,
                    active,
                    &without,
                )?);
            }
        }
        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    fn close_loop(
        &self,
        path: Path,
        full: &PassShape,
        partial: Option<&PassShape>,
        symbols: &BTreeMap<String, Location>,
        registers: &BTreeSet<String>,
        slots: &BTreeSet<i128>,
        latch_place: Place,
    ) -> Result<Vec<Outcome>, Refusal> {
        let refuse = |problem| Refusal::Loop(latch_place, problem);
        let (counters, condition) = full
            .counter
            .clone()
            .ok_or_else(|| refuse(LoopProblem::NoCounter))?;
        if condition.is_unsigned {
            return Err(refuse(LoopProblem::Unsigned));
        }
        let mut beta = 0;
        let mut entries: Vec<(String, Location, Linear, i128)> = Vec::new();
        for (name, coefficient) in &counters {
            let step = full.steps.get(name).copied().unwrap_or(0);
            if step == 0 {
                return Err(refuse(LoopProblem::DependsOnData));
            }
            if full.tally.contains(name) {
                return Err(refuse(LoopProblem::DependsOnOuterCounter));
            }
            let location = symbols
                .get(name)
                .cloned()
                .ok_or_else(|| refuse(LoopProblem::NoCounter))?;
            let initial = match read_location(&path.state, &location) {
                Value::Linear(linear) => linear,
                Value::Stack(_) | Value::Low32(_) | Value::Unknown => {
                    return Err(refuse(LoopProblem::DependsOnData));
                }
            };
            beta += coefficient * step;
            entries.push((name.clone(), location, initial, step));
        }
        if beta == 0 {
            return Err(refuse(LoopProblem::NoCounter));
        }
        let substitution = |values: &BTreeMap<String, Linear>, expression: &Linear| {
            with_identity(expression, values.clone())
        };
        let initial: BTreeMap<String, Linear> = entries
            .iter()
            .map(|(name, _, value, _)| (name.clone(), value.clone()))
            .collect();
        let finals = |values: &BTreeMap<String, Linear>, passes: Option<&Poly>| {
            entries
                .iter()
                .map(|(name, location, _, step)| {
                    let start = values.get(name).cloned();
                    let value = match passes {
                        Some(passes) => passes
                            .as_linear()
                            .zip(start)
                            .map(|(passes, start)| start.plus(&passes.scaled(*step))),
                        None => start,
                    };
                    (location.clone(), value)
                })
                .collect::<Vec<_>>()
        };
        let mut outcomes = Vec::new();
        let mut starts: Vec<(Path, BTreeMap<String, Linear>)> = Vec::new();
        match partial {
            None => starts.push((path.clone(), initial.clone())),
            Some(partial_shape) => {
                let (_, partial_condition) = partial_shape
                    .counter
                    .clone()
                    .ok_or_else(|| refuse(LoopProblem::NoCounter))?;
                let first = partial_condition
                    .substituted(&substitution(&initial, &partial_condition.expression))
                    .ok_or_else(|| refuse(LoopProblem::DependsOnData))?;
                let after_partial: BTreeMap<String, Linear> = entries
                    .iter()
                    .map(|(name, _, value, _)| {
                        let step = partial_shape.steps.get(name).copied().unwrap_or(0);
                        (name.clone(), value.plus_constant(step))
                    })
                    .collect();
                let mut ended = path.clone();
                ended.tally = ended.tally.plus(&partial_shape.tally);
                ended.outside.extend(partial_shape.outside.iter().cloned());
                ended.computed_writes |= partial_shape.computed_writes;
                for (branch, holds) in split(&ended, &first) {
                    if holds {
                        starts.push((branch, after_partial.clone()));
                    } else {
                        outcomes.push(Outcome::Ended(finish(
                            branch,
                            &finals(&after_partial, None),
                            registers,
                            slots,
                        )));
                    }
                }
            }
        }
        for (start_path, start_values) in starts {
            let raw = condition
                .expression
                .substituted(&substitution(&start_values, &condition.expression))
                .ok_or_else(|| refuse(LoopProblem::DependsOnData))?;
            let gamma = raw.numerator();
            let magnitude = beta.abs() * raw.denominator();
            let beta = beta.signum();
            let mut counted: Vec<(Path, Result<Poly, LoopProblem>)> = Vec::new();
            for (class_path, remainder) in
                residue_classes(&start_path, &gamma, magnitude).map_err(refuse)?
            {
                let shifted = gamma.plus_constant(-remainder);
                match condition.relation {
                    Relation::NotZero if remainder != 0 => {
                        counted.push((class_path, Err(LoopProblem::Wraps)));
                    }
                    Relation::Zero if remainder != 0 => {
                        counted.push((class_path, Ok(Poly::constant(1))));
                    }
                    relation => {
                        counted.extend(trip_cases(&class_path, &shifted, magnitude, relation, beta))
                    }
                }
            }
            for (mut case_path, passes) in counted {
                match passes {
                    Ok(passes) => {
                        case_path.tally = case_path.tally.plus(&full.tally.times(&passes));
                        case_path.outside.extend(full.outside.iter().cloned());
                        case_path.computed_writes |= full.computed_writes;
                        outcomes.push(Outcome::Ended(finish(
                            case_path,
                            &finals(&start_values, Some(&passes)),
                            registers,
                            slots,
                        )));
                    }
                    Err(problem) => {
                        let problem = match problem {
                            LoopProblem::Wraps | LoopProblem::NeverEnds
                                if case_path.data_forked =>
                            {
                                LoopProblem::DependsOnData
                            }
                            other => other,
                        };
                        outcomes.push(Outcome::Refused(case_path, refuse(problem)));
                    }
                }
            }
        }
        Ok(outcomes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Location {
    Register(String, CounterWidth),
    Slot(i128),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CounterWidth {
    Full,
    Double,
}

#[derive(Clone, Debug)]
struct PassShape {
    tally: Tally,
    steps: BTreeMap<String, i128>,
    counter: Option<(Vec<(String, i128)>, Condition)>,
    outside: BTreeSet<String>,
    computed_writes: bool,
}

fn with_identity(
    expression: &Linear,
    mut substitution: BTreeMap<String, Linear>,
) -> BTreeMap<String, Linear> {
    for name in expression.terms().keys() {
        substitution
            .entry(name.clone())
            .or_insert_with(|| Linear::symbol(name));
    }
    substitution
}

fn read_location(state: &State, location: &Location) -> Value {
    match location {
        Location::Register(family, width) => {
            let name = match width {
                CounterWidth::Full => name_at(family, Width::Full),
                CounterWidth::Double => name_at(family, Width::Double),
            };
            read(state, &Operand::Register(name))
        }
        Location::Slot(offset) => slot_value(state, *offset),
    }
}

fn slot_value(state: &State, offset: i128) -> Value {
    state
        .slots
        .get(&offset)
        .map_or(Value::Unknown, |(_, value)| value.clone())
}

fn put_slot(state: &mut State, offset: i128, width: i128, value: Value) {
    state.slots.retain(|other, (other_width, _)| {
        *other + *other_width <= offset || offset + width <= *other
    });
    state.slots.insert(offset, (width, value));
}

fn finish(
    mut path: Path,
    counters: &[(Location, Option<Linear>)],
    registers: &BTreeSet<String>,
    slots: &BTreeSet<i128>,
) -> Path {
    for family in registers {
        path.state.registers.insert(family.clone(), Value::Unknown);
    }
    for offset in slots {
        if let Some((_, value)) = path.state.slots.get_mut(offset) {
            *value = Value::Unknown;
        }
    }
    for (counter, final_value) in counters {
        let value = final_value.clone().map_or(Value::Unknown, Value::Linear);
        match counter {
            Location::Register(family, width) => {
                let value = match (width, value) {
                    (CounterWidth::Double, Value::Linear(linear)) => {
                        Value::Linear(linear.as_constant().map_or(linear, |constant| {
                            Linear::constant(unsigned_at(constant, 32))
                        }))
                    }
                    (_, value) => value,
                };
                path.state.registers.insert(family.clone(), value);
            }
            Location::Slot(offset) => {
                if let Some((_, slot)) = path.state.slots.get_mut(offset) {
                    *slot = value;
                }
            }
        }
    }
    path.state.flags = Flags::Unknown;
    path
}

fn split(path: &Path, condition: &Condition) -> Vec<(Path, bool)> {
    match path.bounds.decide(condition) {
        Decision::Holds => vec![(path.clone(), true)],
        Decision::Fails => vec![(path.clone(), false)],
        Decision::Open => {
            let mut branches = Vec::new();
            for (condition, holds) in [(condition.clone(), true), (condition.negated(), false)] {
                if let Some(bounds) = path.bounds.with(&condition) {
                    let mut branch = path.clone();
                    branch.bounds = bounds;
                    branch.conditions.insert(condition);
                    branches.push((branch, holds));
                }
            }
            branches
        }
    }
}

fn residue_classes(
    path: &Path,
    gamma: &Linear,
    modulus: i128,
) -> Result<Vec<(Path, i128)>, LoopProblem> {
    let uneven: Vec<(&String, &i128)> = gamma
        .terms()
        .iter()
        .filter(|(_, coefficient)| *coefficient % modulus != 0)
        .collect();
    let constant = gamma.constant_part();
    match uneven.as_slice() {
        [] => Ok(vec![(path.clone(), constant.rem_euclid(modulus))]),
        [(name, coefficient)] => {
            let needed = modulus / greatest_common_divisor(**coefficient, modulus);
            if needed > RESIDUE_LIMIT {
                return Err(LoopProblem::Step);
            }
            let mut classes = Vec::new();
            for residue in 0..needed {
                let class =
                    Condition::congruent(Linear::symbol(name).plus_constant(-residue), needed);
                let remainder = (**coefficient * residue + constant).rem_euclid(modulus);
                match path.bounds.decide(&class) {
                    Decision::Fails => {}
                    Decision::Holds => classes.push((path.clone(), remainder)),
                    Decision::Open => {
                        if let Some(bounds) = path.bounds.with(&class) {
                            let mut branch = path.clone();
                            branch.bounds = bounds;
                            branch.conditions.insert(class);
                            classes.push((branch, remainder));
                        }
                    }
                }
            }
            Ok(classes)
        }
        _ => Err(LoopProblem::Step),
    }
}

fn greatest_common_divisor(left: i128, right: i128) -> i128 {
    let (mut left, mut right) = (left.abs(), right.abs());
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn trip_cases(
    path: &Path,
    shifted: &Linear,
    divisor: i128,
    relation: Relation,
    beta: i128,
) -> Vec<(Path, Result<Poly, LoopProblem>)> {
    let gamma_poly = Poly::from_linear(shifted).divided(divisor);
    let gamma = shifted;
    let one = Poly::constant(1);
    let two = Poly::constant(2);
    let (test, when_true, when_false): (
        Condition,
        Result<Poly, LoopProblem>,
        Result<Poly, LoopProblem>,
    ) = match (relation, beta) {
        (Relation::AtLeastZero, -1) => (
            Condition::at_least_zero(gamma.plus_constant(divisor)),
            Ok(gamma_poly.plus(&two)),
            Ok(one.clone()),
        ),
        (Relation::AtLeastZero, _) => (
            Condition::at_least_zero(gamma.clone()),
            Err(LoopProblem::NeverEnds),
            Ok(one.clone()),
        ),
        (Relation::NotZero, -1) => (
            Condition::at_least_zero(gamma.clone()),
            Ok(gamma_poly.plus(&one)),
            Err(LoopProblem::Wraps),
        ),
        (Relation::NotZero, _) => (
            Condition::at_least_zero(gamma.scaled(-1)),
            Ok(one.minus(&gamma_poly)),
            Err(LoopProblem::Wraps),
        ),
        (Relation::Zero, _) => (
            Condition::new(gamma.clone(), Relation::Zero, false),
            Ok(two.clone()),
            Ok(one.clone()),
        ),
        (Relation::Congruent(_) | Relation::NotCongruent(_), _) => {
            return vec![(path.clone(), Err(LoopProblem::NoCounter))];
        }
    };
    split(path, &test)
        .into_iter()
        .map(|(branch, holds)| {
            let passes = if holds {
                when_true.clone()
            } else {
                when_false.clone()
            };
            (branch, passes)
        })
        .collect()
}

fn shape(
    outcomes: Vec<Outcome>,
    symbols: &BTreeMap<String, Location>,
    latch: &Instruction,
    continues_when_taken: bool,
    bounds: &Bounds,
) -> Result<PassShape, LoopProblem> {
    let mut ends = Vec::new();
    for outcome in outcomes {
        match outcome {
            Outcome::Ended(path) => ends.push(path),
            Outcome::Refused(_, _) => return Err(LoopProblem::DependsOnData),
        }
    }
    if ends.is_empty() {
        return Err(LoopProblem::NoCounter);
    }
    let mut steps = BTreeMap::new();
    for (symbol, location) in symbols {
        let step = ends
            .iter()
            .map(|end| match read_location(&end.state, location) {
                Value::Linear(linear) => {
                    let difference = linear.minus(&Linear::symbol(symbol));
                    difference.as_constant()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(Some(value)) = step.first()
            && step.iter().all(|other| *other == Some(*value))
            && *value != 0
        {
            steps.insert(symbol.clone(), *value);
        }
    }
    let conditions: Vec<Option<Condition>> = ends
        .iter()
        .map(
            |end| match predicate(&end.state.flags, &latch.mnemonic, &end.bounds) {
                Predicate::Condition(condition) if continues_when_taken => Some(condition),
                Predicate::Condition(condition) => Some(condition.negated()),
                Predicate::Data => None,
            },
        )
        .collect();
    let counter = match conditions.first() {
        Some(Some(condition))
            if conditions
                .iter()
                .all(|other| other.as_ref() == Some(condition)) =>
        {
            let counters: Vec<(&String, &i128)> = condition
                .expression
                .terms()
                .iter()
                .filter(|(name, _)| symbols.contains_key(*name))
                .collect();
            if counters.is_empty() {
                None
            } else {
                Some((
                    counters
                        .iter()
                        .map(|(name, coefficient)| ((*name).clone(), **coefficient))
                        .collect(),
                    condition.clone(),
                ))
            }
        }
        Some(_) => return Err(LoopProblem::DependsOnData),
        None => None,
    };
    let tally = merge_tallies(ends.iter().map(|end| end.tally.clone()).collect(), bounds)
        .ok_or(LoopProblem::DependsOnData)?;
    let mut outside = BTreeSet::new();
    let mut computed_writes = false;
    for end in &ends {
        outside.extend(end.outside.iter().cloned());
        computed_writes |= end.computed_writes;
    }
    Ok(PassShape {
        tally,
        steps,
        counter,
        outside,
        computed_writes,
    })
}

fn merge_ranges(ranges: &[Range], bounds: &Bounds) -> Option<Range> {
    let first = ranges.first()?.clone();
    let mut low = first.low.clone();
    let mut high = first.high.clone();
    for range in &ranges[1..] {
        if range.low.is_at_most(&low, bounds) {
            low = range.low.clone();
        } else if !low.is_at_most(&range.low, bounds) {
            return None;
        }
        if high.is_at_most(&range.high, bounds) {
            high = range.high.clone();
        } else if !range.high.is_at_most(&high, bounds) {
            return None;
        }
    }
    Some(Range { low, high })
}

fn merge_tallies(tallies: Vec<Tally>, bounds: &Bounds) -> Option<Tally> {
    let pick = |select: fn(&Tally) -> Range| -> Option<Range> {
        merge_ranges(&tallies.iter().map(select).collect::<Vec<_>>(), bounds)
    };
    Some(Tally {
        instructions: pick(|tally| tally.instructions.clone())?,
        reads: pick(|tally| tally.reads.clone())?,
        writes: pick(|tally| tally.writes.clone())?,
        memory_known: tallies.iter().all(|tally| tally.memory_known),
    })
}

fn condition_is_constant(condition: &Condition) -> bool {
    condition.expression.as_constant().is_some()
}

fn count(path: &mut Path, instruction: &Instruction) {
    let one = Range::exact(Poly::constant(1));
    path.tally.instructions = path.tally.instructions.plus(&one);
    match accesses(instruction) {
        Some(counted) => {
            path.tally.reads = path
                .tally
                .reads
                .plus(&Range::exact(Poly::constant(counted.reads)));
            path.tally.writes = path
                .tally
                .writes
                .plus(&Range::exact(Poly::constant(counted.writes)));
        }
        None => path.tally.memory_known = false,
    }
}

fn clobber(state: &mut State) {
    for family in CALLER_SAVED {
        state.registers.insert(family.to_owned(), Value::Unknown);
    }
    state.flags = Flags::Unknown;
}

fn predicate(flags: &Flags, mnemonic: &str, bounds: &Bounds) -> Predicate {
    if let Flags::Residue(value, modulus) = flags {
        return match (mnemonic, *modulus) {
            ("je" | "jz", _) => Predicate::Condition(Condition::congruent(value.clone(), *modulus)),
            ("jne" | "jnz", 2) => {
                Predicate::Condition(Condition::congruent(value.plus_constant(1), 2))
            }
            _ => Predicate::Data,
        };
    }
    let unsigned_jump = matches!(
        mnemonic,
        "jb" | "jc" | "jnae" | "jbe" | "jna" | "ja" | "jnbe" | "jae" | "jnb" | "jnc"
    );
    if let Flags::ResultWithoutComparedCarry(value) = flags {
        if unsigned_jump {
            return Predicate::Data;
        }
        return predicate(&result_flags(value), mnemonic, bounds);
    }
    let Flags::Compare(Value::Linear(left), Value::Linear(right), bits) = flags else {
        return Predicate::Data;
    };
    if let (Some(left_value), Some(right_value)) = (left.as_constant(), right.as_constant()) {
        let (left_value, right_value) = if unsigned_jump {
            (
                unsigned_at(left_value, *bits),
                unsigned_at(right_value, *bits),
            )
        } else {
            (
                signed_value(left_value, *bits),
                signed_value(right_value, *bits),
            )
        };
        let holds = match mnemonic {
            "je" | "jz" => left_value == right_value,
            "jne" | "jnz" => left_value != right_value,
            "jl" | "jnge" | "jb" | "jc" | "jnae" => left_value < right_value,
            "jle" | "jng" | "jbe" | "jna" => left_value <= right_value,
            "jg" | "jnle" | "ja" | "jnbe" => left_value > right_value,
            "jge" | "jnl" | "jae" | "jnb" | "jnc" => left_value >= right_value,
            "js" => left_value - right_value < 0,
            "jns" => left_value - right_value >= 0,
            _ => return Predicate::Data,
        };
        return Predicate::Condition(Condition::at_least_zero(Linear::constant(if holds {
            0
        } else {
            -1
        })));
    }
    let difference = left.minus(right);
    let below = Condition::at_least_zero(difference.scaled(-1).plus_constant(-1));
    let at_most = Condition::at_least_zero(difference.scaled(-1));
    let above = Condition::at_least_zero(difference.plus_constant(-1));
    let at_least = Condition::at_least_zero(difference.clone());
    let unsigned =
        |condition: Condition| Condition::new(condition.expression, condition.relation, true);
    let condition = match mnemonic {
        "je" | "jz" => Condition::new(difference, Relation::Zero, false),
        "jne" | "jnz" => Condition::new(difference, Relation::NotZero, false),
        "jl" | "jnge" | "js" => below,
        "jle" | "jng" => at_most,
        "jg" | "jnle" => above,
        "jge" | "jnl" | "jns" => at_least,
        "jb" | "jc" | "jnae" => unsigned(below),
        "jbe" | "jna" => unsigned(at_most),
        "ja" | "jnbe" => unsigned(above),
        "jae" | "jnb" | "jnc" => unsigned(at_least),
        _ => return Predicate::Data,
    };
    let nonnegative =
        |value: &Linear| bounds.decide(&Condition::at_least_zero(value.clone())) == Decision::Holds;
    if condition.is_unsigned && nonnegative(left) && nonnegative(right) {
        return Predicate::Condition(Condition::new(
            condition.expression,
            condition.relation,
            false,
        ));
    }
    Predicate::Condition(condition)
}

fn read(state: &State, operand: &Operand) -> Value {
    match operand {
        Operand::Register(name) => {
            let Some(found) = register(name) else {
                return Value::Unknown;
            };
            if found.width == Width::Partial {
                return match (low_mask(name), family_value(state, &found.family)) {
                    (Some(mask), Value::Linear(linear)) => {
                        linear.as_constant().map_or(Value::Unknown, |value| {
                            Value::Linear(Linear::constant(value & mask))
                        })
                    }
                    _ => Value::Unknown,
                };
            }
            let value = match state.registers.get(&found.family) {
                Some(value) => value.clone(),
                None if is_general(&found.family) => {
                    let symbol = entry_symbol(state, &found.family);
                    let entry = Linear::symbol(&symbol);
                    if found.width == Width::Full && symbol == name_at(&found.family, Width::Double)
                    {
                        Value::Low32(entry)
                    } else {
                        Value::Linear(entry)
                    }
                }
                None => Value::Unknown,
            };
            match (&value, found.width) {
                (Value::Linear(linear), Width::Double) => linear.as_constant().map_or_else(
                    || Value::Linear(low_halves(linear)),
                    |constant| Value::Linear(Linear::constant(signed_value(constant, 32))),
                ),
                (Value::Low32(linear), Width::Double) => Value::Linear(linear.clone()),
                _ => value,
            }
        }
        Operand::Immediate(value) => Value::Linear(Linear::constant(*value)),
        Operand::Memory(memory) => match address(state, memory) {
            Some(Value::Stack(offset)) => slot_value(state, offset),
            _ => Value::Unknown,
        },
        Operand::Target(_) | Operand::Address { .. } => Value::Unknown,
    }
}

fn known_residue(bounds: &Bounds, value: &Linear, modulus: i128) -> Option<i128> {
    if value.denominator() != 1 {
        return None;
    }
    if let Some(constant) = value.as_constant() {
        return Some(constant.rem_euclid(modulus));
    }
    (0..modulus).find(|residue| {
        bounds.decide(&Condition::congruent(
            value.plus_constant(-residue),
            modulus,
        )) == Decision::Holds
    })
}

fn split_residue(path: &Path, instruction: &Instruction) -> Option<Vec<Path>> {
    let destination = instruction.operands.first()?;
    let Operand::Immediate(amount) = instruction.operands.get(1)? else {
        return None;
    };
    let Operand::Register(name) = destination else {
        return None;
    };
    if register(name)?.width == Width::Partial {
        return None;
    }
    let mnemonic = instruction.mnemonic.as_str();
    let modulus = match mnemonic {
        "shr" | "sar" if (1..=4).contains(amount) => 1_i128 << amount,
        "and" if *amount < 0 && (-amount) <= RESIDUE_LIMIT && (-amount) & (-amount - 1) == 0 => {
            -amount
        }
        "and" | "test" if *amount > 0 && *amount < RESIDUE_LIMIT => {
            if mnemonic == "test" && (amount + 1) & amount == 0 {
                return None;
            }
            1_i128 << (128 - amount.leading_zeros())
        }
        _ => return None,
    };
    let Value::Linear(value) = read(&path.state, destination) else {
        return None;
    };
    if value.as_constant().is_some()
        || value.denominator() != 1
        || value.terms().keys().any(|name| is_counter(name))
    {
        return None;
    }
    if mnemonic == "shr"
        && path.bounds.decide(&Condition::at_least_zero(value.clone())) != Decision::Holds
    {
        return None;
    }
    let classes: Vec<(Path, i128)> = match known_residue(&path.bounds, &value, modulus) {
        Some(residue) => vec![(path.clone(), residue)],
        None => (0..modulus)
            .filter_map(|residue| {
                let class = Condition::congruent(value.plus_constant(-residue), modulus);
                let bounds = path.bounds.with(&class)?;
                let mut branch = path.clone();
                branch.bounds = bounds;
                branch.conditions.insert(class);
                Some((branch, residue))
            })
            .collect(),
    };
    Some(
        classes
            .into_iter()
            .map(|(mut branch, residue)| {
                let result = match mnemonic {
                    "shr" | "sar" => value.plus_constant(-residue).divided(modulus),
                    _ if *amount < 0 => value.plus_constant(-residue),
                    _ => Linear::constant(residue & amount),
                };
                let result = Value::Linear(result);
                branch.state.flags = result_flags(&result);
                if mnemonic != "test" {
                    store(&mut branch.state, destination, result);
                }
                branch
            })
            .collect(),
    )
}

fn constant_of(state: &State, operand: &Operand) -> Option<i128> {
    match read(state, operand) {
        Value::Linear(linear) => linear.as_constant(),
        _ => None,
    }
}

fn low_mask(name: &str) -> Option<i128> {
    let name = name.to_ascii_lowercase();
    if name.ends_with('h') && name.len() == 2 {
        return None;
    }
    if name.ends_with('l') || name.ends_with('b') {
        Some(0xff)
    } else {
        Some(0xffff)
    }
}

fn family_value(state: &State, family: &str) -> Value {
    match state.registers.get(family) {
        Some(value) => value.clone(),
        None if is_general(family) => Value::Linear(Linear::symbol(&entry_symbol(state, family))),
        None => Value::Unknown,
    }
}

fn register_value_for_mask(state: &State, name: &str) -> Value {
    match register(name) {
        Some(found) if found.width == Width::Partial && low_mask(name).is_none() => Value::Unknown,
        Some(found) => family_value(state, &found.family),
        None => Value::Unknown,
    }
}

fn is_register_nop(instruction: &Instruction) -> bool {
    matches!(instruction.mnemonic.as_str(), "xchg")
        && matches!(
            (instruction.operands.first(), instruction.operands.get(1)),
            (Some(Operand::Register(left)), Some(Operand::Register(right))) if left == right
        )
}

fn address(state: &State, memory: &Memory) -> Option<Value> {
    if memory.has_symbol {
        return None;
    }
    let base = match &memory.base {
        Some(name) => read(state, &Operand::Register(name.clone())),
        None => Value::Linear(Linear::constant(0)),
    };
    let index = memory
        .index
        .as_ref()
        .map(|name| read(state, &Operand::Register(name.clone())));
    let sum = |base: &Linear, index: Option<&Linear>| {
        index
            .map_or(base.clone(), |index| base.plus(&index.scaled(memory.scale)))
            .plus_constant(memory.displacement)
    };
    match (base, index) {
        (Value::Stack(offset), None) => Some(Value::Stack(offset + memory.displacement)),
        (Value::Linear(base), None) => Some(Value::Linear(sum(&base, None))),
        (Value::Linear(base), Some(Value::Linear(index))) => {
            Some(Value::Linear(sum(&base, Some(&index))))
        }
        (Value::Low32(base), None) => Some(Value::Low32(sum(&base, None))),
        (Value::Low32(base), Some(Value::Linear(index) | Value::Low32(index)))
        | (Value::Linear(base), Some(Value::Low32(index))) => {
            Some(Value::Low32(sum(&base, Some(&index))))
        }
        _ => None,
    }
}

fn operand_width(operand: Option<&Operand>) -> Option<i128> {
    match operand {
        Some(Operand::Register(name)) => register(name).map(|found| match found.width {
            Width::Full => 8,
            Width::Double => 4,
            Width::Partial if low_mask(name) == Some(0xffff) => 2,
            Width::Partial => 1,
        }),
        Some(Operand::Memory(memory)) => memory.size,
        _ => None,
    }
}

fn store(state: &mut State, operand: &Operand, value: Value) -> bool {
    store_sized(state, operand, value, None)
}

fn store_sized(state: &mut State, operand: &Operand, value: Value, width: Option<i128>) -> bool {
    match operand {
        Operand::Register(name) => {
            if let Some(found) = register(name) {
                let value = match (found.width, value) {
                    (Width::Partial, _) => Value::Unknown,
                    (Width::Double, Value::Linear(linear) | Value::Low32(linear)) => {
                        match linear.as_constant() {
                            Some(constant) => {
                                Value::Linear(Linear::constant(unsigned_at(constant, 32)))
                            }
                            None if fits(&linear, 31) => Value::Linear(linear),
                            None => Value::Unknown,
                        }
                    }
                    (Width::Full, Value::Linear(linear))
                        if linear.as_constant().is_none() && !fits(&linear, 63) =>
                    {
                        Value::Unknown
                    }
                    (_, value) => value,
                };
                let narrow_symbol = found.width == Width::Double
                    && matches!(&value, Value::Linear(linear) if linear.as_constant().is_none());
                if narrow_symbol {
                    state.zero_extended.insert(found.family.clone());
                } else {
                    state.zero_extended.remove(&found.family);
                }
                state.registers.insert(found.family, value);
            }
            false
        }
        Operand::Memory(memory) => match address(state, memory) {
            Some(Value::Stack(offset)) => {
                let width = memory.size.or(width).unwrap_or(SLOT_WIDTH);
                put_slot(state, offset, width, value);
                false
            }
            _ => true,
        },
        _ => false,
    }
}

fn arithmetic(left: &Value, right: &Value, sign: i128) -> Value {
    match (left, right) {
        (Value::Linear(left), Value::Linear(right)) => {
            Value::Linear(left.plus(&right.scaled(sign)))
        }
        (Value::Low32(left), Value::Linear(right) | Value::Low32(right))
        | (Value::Linear(left), Value::Low32(right)) => {
            Value::Low32(left.plus(&right.scaled(sign)))
        }
        (Value::Stack(offset), Value::Linear(right)) => right
            .as_constant()
            .map_or(Value::Unknown, |value| Value::Stack(offset + sign * value)),
        _ => Value::Unknown,
    }
}

fn result_flags(value: &Value) -> Flags {
    Flags::Compare(value.clone(), Value::Linear(Linear::constant(0)), 64)
}

fn fits(value: &Linear, bits: u32) -> bool {
    let limit = 1_i128 << bits;
    value.constant_part().abs() < limit
        && value.denominator() < limit
        && value
            .terms()
            .values()
            .all(|coefficient| coefficient.abs() < limit)
}

fn signed_value(value: i128, bits: u32) -> i128 {
    let span = 1_i128 << bits;
    let value = value.rem_euclid(span);
    if value >= span / 2 {
        value - span
    } else {
        value
    }
}

fn unsigned_at(value: i128, bits: u32) -> i128 {
    value.rem_euclid(1_i128 << bits)
}

fn operand_bits(operand: &Operand) -> Option<u32> {
    match operand {
        Operand::Register(name) => match register(name)?.width {
            Width::Full => Some(64),
            Width::Double => Some(32),
            Width::Partial => Some(
                if low_mask(name) == Some(0xff) || name.len() == 2 && name.ends_with('h') {
                    8
                } else {
                    16
                },
            ),
        },
        Operand::Memory(memory) => memory.size.and_then(|size| u32::try_from(size * 8).ok()),
        _ => None,
    }
}

fn step(state: &mut State, instruction: &Instruction) -> bool {
    let operands = &instruction.operands;
    let first = operands.first();
    let second = operands.get(1);
    let mnemonic = instruction.mnemonic.as_str();
    match (mnemonic, first, second) {
        (
            "nop" | "endbr64" | "endbr32" | "pause" | "prefetcht0" | "prefetcht1" | "prefetcht2"
            | "prefetchnta" | "prefetchw",
            _,
            _,
        ) => false,
        (
            "mov" | "movzx" | "movsx" | "movsxd" | "movabs" | "cdqe",
            Some(destination),
            Some(source),
        ) => {
            let source_width = operand_width(Some(source));
            let value = match (mnemonic, read(state, source), source_width) {
                ("movsx" | "movsxd", Value::Linear(linear), Some(bytes)) if bytes < 8 => {
                    match linear.as_constant() {
                        Some(constant) => {
                            let bits = u32::try_from(bytes * 8).unwrap_or(64);
                            Value::Linear(Linear::constant(signed_value(constant, bits)))
                        }
                        None if bytes == 4 => Value::Linear(linear),
                        None => Value::Unknown,
                    }
                }
                (_, value, _) => value,
            };
            store_sized(state, destination, value, source_width)
        }
        ("lea", Some(destination), Some(Operand::Memory(memory))) => {
            let value = address(state, memory).unwrap_or(Value::Unknown);
            store(state, destination, value)
        }
        ("xor" | "sub", Some(Operand::Register(left)), Some(Operand::Register(right)))
            if register(left).map(|found| found.family)
                == register(right).map(|found| found.family) =>
        {
            let zero = Value::Linear(Linear::constant(0));
            state.flags = result_flags(&zero);
            store(state, &Operand::Register(left.clone()), zero)
        }
        ("sub", Some(destination), Some(source)) => {
            let before = read(state, destination);
            let subtrahend = read(state, source);
            let bits = operand_bits(destination)
                .or_else(|| operand_bits(source))
                .unwrap_or(64);
            let value = arithmetic(&before, &subtrahend, -1);
            state.flags = Flags::Compare(before, subtrahend, bits);
            store(state, destination, value)
        }
        ("add", Some(destination), Some(source)) => {
            let value = arithmetic(&read(state, destination), &read(state, source), 1);
            state.flags = Flags::ResultWithoutComparedCarry(value.clone());
            store(state, destination, value)
        }
        ("inc" | "dec", Some(destination), None) => {
            let sign = if mnemonic == "inc" { 1 } else { -1 };
            let value = arithmetic(
                &read(state, destination),
                &Value::Linear(Linear::constant(1)),
                sign,
            );
            state.flags = Flags::ResultWithoutComparedCarry(value.clone());
            store(state, destination, value)
        }
        ("neg", Some(destination), None) => {
            let value = match read(state, destination) {
                Value::Linear(linear) => Value::Linear(linear.scaled(-1)),
                _ => Value::Unknown,
            };
            state.flags = Flags::ResultWithoutComparedCarry(value.clone());
            store(state, destination, value)
        }
        ("imul", Some(destination), Some(source)) => {
            let factor = operands
                .get(2)
                .map_or_else(|| read(state, destination), |third| read(state, third));
            let value = match (read(state, source), factor) {
                (Value::Linear(left), Value::Linear(right)) => {
                    match (left.as_constant(), right.as_constant()) {
                        (Some(constant), _) => Value::Linear(right.scaled(constant)),
                        (_, Some(constant)) => Value::Linear(left.scaled(constant)),
                        _ => Value::Unknown,
                    }
                }
                _ => Value::Unknown,
            };
            state.flags = Flags::Unknown;
            store(state, destination, value)
        }
        ("shr" | "sar" | "and" | "or" | "xor", Some(destination), Some(source))
            if constant_of(state, destination).is_some()
                && constant_of(state, source).is_some() =>
        {
            let (Some(left), Some(right)) =
                (constant_of(state, destination), constant_of(state, source))
            else {
                return false;
            };
            let bits = match destination {
                Operand::Register(name) => match register(name).map(|found| found.width) {
                    Some(Width::Double) => 32,
                    _ => 64,
                },
                _ => 64,
            };
            let span = 1_i128 << bits;
            let wrapped = |value: i128| {
                let value = value.rem_euclid(span);
                if value >= span / 2 {
                    value - span
                } else {
                    value
                }
            };
            let value = match mnemonic {
                "shr" if (0..64).contains(&right) => wrapped(left.rem_euclid(span) >> right),
                "sar" if (0..64).contains(&right) => left >> right,
                "and" => wrapped(left & right),
                "or" => wrapped(left | right),
                "xor" => wrapped(left ^ right),
                _ => return store(state, destination, Value::Unknown),
            };
            let value = Value::Linear(Linear::constant(value));
            state.flags = result_flags(&value);
            store(state, destination, value)
        }
        ("shl" | "sal", Some(destination), Some(Operand::Immediate(shift)))
            if (0..63).contains(shift) =>
        {
            let value = match read(state, destination) {
                Value::Linear(linear) => Value::Linear(linear.scaled(1_i128 << shift)),
                _ => Value::Unknown,
            };
            state.flags = Flags::Unknown;
            store(state, destination, value)
        }
        ("cmp", Some(left), Some(right)) => {
            let bits = operand_bits(left)
                .or_else(|| operand_bits(right))
                .unwrap_or(64);
            state.flags = Flags::Compare(read(state, left), read(state, right), bits);
            false
        }
        ("xchg", _, _) if is_register_nop(instruction) => false,
        ("test" | "and", Some(Operand::Register(name)), Some(Operand::Immediate(mask))) => {
            let value = register_value_for_mask(state, name);
            let constant = match &value {
                Value::Linear(linear) => linear.as_constant(),
                _ => None,
            };
            let result = constant.map(|constant| Value::Linear(Linear::constant(constant & mask)));
            state.flags = match (&result, &value) {
                (Some(result), _) => result_flags(result),
                (None, Value::Linear(linear))
                    if *mask > 0 && *mask < 256 && (mask + 1) & mask == 0 =>
                {
                    Flags::Residue(linear.clone(), mask + 1)
                }
                _ => Flags::Unknown,
            };
            if mnemonic == "and" {
                let stored = result.unwrap_or(Value::Unknown);
                return store(state, &Operand::Register(name.clone()), stored);
            }
            false
        }
        ("test", Some(Operand::Register(left)), Some(Operand::Register(right)))
            if left == right =>
        {
            state.flags = result_flags(&read(state, &Operand::Register(left.clone())));
            false
        }
        ("push", Some(source), _) => {
            let value = read(state, source);
            let rsp = arithmetic(
                &read(state, &Operand::Register("rsp".to_owned())),
                &Value::Linear(Linear::constant(STACK_STEP)),
                -1,
            );
            state.registers.insert("rsp".to_owned(), rsp.clone());
            if let Value::Stack(offset) = rsp {
                put_slot(state, offset, STACK_STEP, value);
            }
            false
        }
        ("pop", Some(destination), _) => {
            let rsp = read(state, &Operand::Register("rsp".to_owned()));
            let value = match rsp {
                Value::Stack(offset) => slot_value(state, offset),
                _ => Value::Unknown,
            };
            let next = arithmetic(&rsp, &Value::Linear(Linear::constant(STACK_STEP)), 1);
            state.registers.insert("rsp".to_owned(), next);
            store(state, destination, value)
        }
        ("leave" | "leaveq", _, _) => {
            let rbp = read(state, &Operand::Register("rbp".to_owned()));
            state.registers.insert("rsp".to_owned(), rbp);
            step(
                state,
                &Instruction {
                    place: instruction.place,
                    prefix: None,
                    mnemonic: "pop".to_owned(),
                    operands: vec![Operand::Register("rbp".to_owned())],
                },
            )
        }
        ("cdqe", None, None) => {
            let value = read(state, &Operand::Register("eax".to_owned()));
            state.registers.insert("rax".to_owned(), value);
            false
        }
        ("mul" | "div" | "idiv" | "cdq" | "cqo" | "cpuid" | "rdtsc", _, _) => {
            state.registers.insert("rax".to_owned(), Value::Unknown);
            state.registers.insert("rdx".to_owned(), Value::Unknown);
            state.flags = Flags::Unknown;
            false
        }
        ("xchg", Some(left), Some(right)) => {
            let left_value = read(state, left);
            let right_value = read(state, right);
            let first_write = store(state, left, right_value);
            let second_write = store(state, right, left_value);
            first_write || second_write
        }
        _ => {
            let computed = match first {
                Some(destination) if writes_destination(mnemonic) => {
                    store(state, destination, Value::Unknown)
                }
                _ => false,
            };
            if !mnemonic.starts_with("mov") {
                state.flags = Flags::Unknown;
            }
            computed
        }
    }
}

fn case_conditions(conditions: &BTreeSet<Condition>) -> Vec<String> {
    let mut bounds = Bounds::default();
    let mut others = Vec::new();
    for condition in conditions {
        if let Some(tightened) = bounds.with(condition) {
            bounds = tightened;
        }
        let single = condition.expression.terms().len() == 1 && !condition.is_unsigned;
        let bounded = single
            && matches!(
                condition.relation,
                Relation::AtLeastZero | Relation::Zero | Relation::NotZero | Relation::Congruent(_)
            );
        if !bounded {
            others.push(condition.text());
        }
    }
    let mut texts = bounds.texts();
    let others: Vec<String> = conditions
        .iter()
        .filter(|condition| {
            others.contains(&condition.text()) && bounds.decide(condition) != Decision::Holds
        })
        .map(Condition::text)
        .collect();
    texts.extend(others);
    texts.dedup();
    texts
}

type Group = (BTreeSet<Condition>, Bounds, Vec<Tally>, Option<Refusal>);
type ComputedCase = (BTreeSet<Condition>, Result<Counts, Refusal>);

fn may_overlap(first: &Group, second: &Group) -> bool {
    if first
        .0
        .iter()
        .any(|condition| second.0.contains(&condition.negated()))
    {
        return false;
    }
    let excluded = |bounds: &Bounds, conditions: &BTreeSet<Condition>| {
        conditions
            .iter()
            .any(|condition| bounds.decide(condition) == Decision::Fails)
    };
    !excluded(&first.1, &second.0) && !excluded(&second.1, &first.0)
}

fn widened_by_overlaps(groups: Vec<Group>) -> Vec<Group> {
    let mut widened = groups.clone();
    for (index, group) in groups.iter().enumerate() {
        for (other_index, other) in groups.iter().enumerate() {
            if index == other_index || !may_overlap(group, other) {
                continue;
            }
            let own = &mut widened[index];
            if own.3.is_some() {
                continue;
            }
            match &other.3 {
                Some(_) => own.3 = Some(Refusal::DataRange),
                None => {
                    for tally in &other.2 {
                        if !own.2.contains(tally) {
                            own.2.push(tally.clone());
                        }
                    }
                }
            }
        }
    }
    widened
}

fn merged_cases(
    mut cases: Vec<(BTreeSet<Condition>, Result<Counts, Refusal>)>,
) -> Vec<(BTreeSet<Condition>, Result<Counts, Refusal>)> {
    loop {
        let mut pair = None;
        'search: for (first, (first_conditions, first_outcome)) in cases.iter().enumerate() {
            for (second, (second_conditions, second_outcome)) in
                cases.iter().enumerate().skip(first + 1)
            {
                if first_outcome != second_outcome {
                    continue;
                }
                let only_first: Vec<&Condition> =
                    first_conditions.difference(second_conditions).collect();
                let only_second: Vec<&Condition> =
                    second_conditions.difference(first_conditions).collect();
                if let ([left], [right]) = (only_first.as_slice(), only_second.as_slice())
                    && left.negated() == **right
                {
                    let common: BTreeSet<Condition> = first_conditions
                        .intersection(second_conditions)
                        .cloned()
                        .collect();
                    pair = Some((first, second, common));
                    break 'search;
                }
            }
        }
        let Some((first, second, common)) = pair else {
            return cases;
        };
        let outcome = cases[first].1.clone();
        let _ = cases.remove(second);
        cases[first] = (common, outcome);
    }
}

fn count_text(range: &Range) -> Count {
    Count {
        low: range.low.text(),
        high: range.high.text(),
    }
}

fn answered_value_by_value(
    program: &Program,
    function: &Function,
    given: &BTreeMap<String, i128>,
    conditions: &BTreeSet<Condition>,
    bounds: &Bounds,
) -> Option<Vec<ComputedCase>> {
    let names: BTreeSet<&String> = conditions
        .iter()
        .flat_map(|condition| condition.expression.terms().keys())
        .collect();
    let [name] = names.into_iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    let (lowest, highest) = (bounds.lower(name)?, bounds.upper(name)?);
    if highest - lowest >= VALUES_ANSWERED_ONE_BY_ONE {
        return None;
    }
    let mut answered = Vec::new();
    for value in lowest..=highest {
        let at_value = BTreeMap::from([(name.clone(), Linear::constant(value))]);
        let mut holds = true;
        for condition in conditions {
            match Bounds::default().decide(&condition.substituted(&at_value)?) {
                Decision::Holds => {}
                Decision::Fails => holds = false,
                Decision::Open => return None,
            }
        }
        if !holds {
            continue;
        }
        let mut with_value = given.clone();
        with_value.insert(name.clone(), value);
        let report = analyze_function(program, function, &with_value);
        let Ok(cases) = report.outcome else {
            return None;
        };
        let [case] = &cases[..] else {
            return None;
        };
        if !case.conditions.is_empty() {
            return None;
        }
        let equal = Condition::new(
            Linear::symbol(name).plus_constant(-value),
            Relation::Zero,
            false,
        );
        answered.push((BTreeSet::from([equal]), case.outcome.clone()));
    }
    Some(answered)
}

pub fn analyze(program: &Program, given: &BTreeMap<String, i128>) -> Vec<FunctionReport> {
    program
        .functions
        .iter()
        .map(|function| analyze_function(program, function, given))
        .collect()
}

fn analyze_function(
    program: &Program,
    function: &Function,
    given: &BTreeMap<String, i128>,
) -> FunctionReport {
    let walker = Walker::new(program, function, vec![function.name.clone()]);
    let path = Path {
        state: entry_state(function, given),
        bounds: int_ranges(function),
        conditions: BTreeSet::new(),
        tally: Tally::zero(),
        outside: BTreeSet::new(),
        computed_writes: false,
        resume: None,
        data_forked: false,
    };
    let outcomes = match walker.walk(0, path, &[], Stop::Return) {
        Ok(outcomes) => outcomes,
        Err(refusal) => {
            return FunctionReport {
                name: function.name.clone(),
                outcome: Err(refusal),
                inputs: Vec::new(),
                outside: Vec::new(),
                assumes_separate_stack: false,
                has_loops: !walker.regions.is_empty(),
            };
        }
    };
    let mut groups: Vec<Group> = Vec::new();
    let mut outside = BTreeSet::new();
    let mut assumes_separate_stack = false;
    for outcome in outcomes {
        let (path, refusal) = match outcome {
            Outcome::Ended(path) => (path, None),
            Outcome::Refused(path, refusal) => (path, Some(refusal)),
        };
        outside.extend(path.outside.iter().cloned());
        assumes_separate_stack |= path.computed_writes && !walker.regions.is_empty();
        let shown = case_conditions(&path.conditions);
        match groups
            .iter_mut()
            .find(|(conditions, ..)| case_conditions(conditions) == shown)
        {
            Some((_, _, tallies, existing)) => match (refusal, existing.as_ref()) {
                (Some(new), Some(known)) if new != *known => *existing = Some(Refusal::DataRange),
                (Some(_), Some(_)) => {}
                (Some(new), None) => {
                    *existing = Some(if tallies.is_empty() {
                        new
                    } else {
                        Refusal::DataRange
                    });
                }
                (None, Some(_)) => *existing = Some(Refusal::DataRange),
                (None, None) => tallies.push(path.tally),
            },
            None => {
                let tallies = if refusal.is_some() {
                    Vec::new()
                } else {
                    vec![path.tally]
                };
                groups.push((path.conditions, path.bounds, tallies, refusal));
            }
        }
    }
    let groups = widened_by_overlaps(groups);
    let mut inputs = BTreeSet::new();
    let computed: Vec<(BTreeSet<Condition>, Bounds, Result<Counts, Refusal>)> = groups
        .into_iter()
        .map(|(conditions, bounds, tallies, refusal)| {
            for condition in &conditions {
                inputs.extend(condition.expression.terms().keys().cloned());
            }
            let outcome = match refusal {
                Some(refusal) => Err(refusal),
                None => match merge_tallies(tallies, &bounds) {
                    Some(tally) => {
                        for range in [&tally.instructions, &tally.reads, &tally.writes] {
                            inputs.extend(range.low.symbols());
                            inputs.extend(range.high.symbols());
                        }
                        Ok(Counts {
                            instructions: count_text(&tally.instructions),
                            reads: tally.memory_known.then(|| count_text(&tally.reads)),
                            writes: tally.memory_known.then(|| count_text(&tally.writes)),
                        })
                    }
                    None => Err(Refusal::DataRange),
                },
            };
            (conditions, bounds, outcome)
        })
        .collect();
    let computed = computed
        .into_iter()
        .flat_map(|(conditions, bounds, outcome)| match outcome {
            Err(refusal) => answered_value_by_value(program, function, given, &conditions, &bounds)
                .unwrap_or_else(|| vec![(conditions, Err(refusal))]),
            answered => vec![(conditions, answered)],
        })
        .fold(Vec::new(), |mut distinct, case| {
            if !distinct.contains(&case) {
                distinct.push(case);
            }
            distinct
        });
    let cases = merged_cases(computed)
        .into_iter()
        .map(|(conditions, outcome)| Case {
            conditions: case_conditions(&conditions),
            outcome,
        })
        .collect();
    FunctionReport {
        name: function.name.clone(),
        outcome: Ok(cases),
        inputs: inputs
            .into_iter()
            .filter(|name| !is_counter(name))
            .collect(),
        outside: outside.into_iter().collect(),
        assumes_separate_stack,
        has_loops: !walker.regions.is_empty(),
    }
}
