//! Versioned, source-preserving bytecode artifacts with a bounded hostile-input loader.
//!
//! The format uses explicit tags and little-endian integers, never Rust layouts.
//! Decode validates framing, budgets, UTF-8 spans, and the complete bytecode module.

use larp_bytecode::{
    Class, Extension, Field, Function, Implementation, Instruction, Method, Module, Role,
    RoleMethod, ScenarioContext, VerifiedModule, VerifyError,
};
use larp_source::{SourceFile, SourceId, Span};
use larp_types::{Effects, Type};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::mem::size_of;
use std::rc::Rc;

mod codec;

pub const MAGIC: &[u8; 8] = b"LARPBC\r\n";
pub const VERSION: u16 = 2;
/// Wire versions accepted by the decoder, independently of the toolchain version.
pub const SUPPORTED_VERSIONS: &[u16] = &[1, VERSION];
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ALLOCATED: usize = 64 * 1024 * 1024;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_STRINGS: usize = 8 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOURCE_LINES: usize = 131_072;
const MAX_SOURCES: usize = 64;
const MAX_DECLARATIONS: usize = 4096;
const MAX_MEMBERS: usize = 65_536;
const MAX_TYPES: usize = 65_536;
const MAX_TYPE_REFERENCES: usize = 262_144;
const MAX_TYPE_WORK: usize = 2_000_000;
const MAX_INSTRUCTIONS: usize = 262_144;
const MAX_LOCALS: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidFormat,
    ResourceLimit,
    InvalidBytecode,
    Io,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    offset: Option<usize>,
    message: String,
    verification: Option<VerifyError>,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
    pub fn verification(&self) -> Option<&VerifyError> {
        self.verification.as_ref()
    }
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            offset: None,
            message: message.into(),
            verification: None,
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidFormat, message)
    }
    fn resource(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::ResourceLimit, message)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(offset) = self.offset {
            write!(f, " at byte {offset}")?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {}
pub type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Artifact {
    pub module: VerifiedModule,
    pub sources: Vec<SourceFile>,
}
impl Artifact {
    pub fn source(&self, span: Span) -> Option<&SourceFile> {
        self.sources
            .iter()
            .find(|source| source.slice(span).is_some())
    }
}

#[derive(Default)]
struct Budget {
    allocated: usize,
    strings: usize,
    source_bytes: usize,
    source_lines: usize,
    members: usize,
    type_references: usize,
    type_work: usize,
    instructions: usize,
    locals: usize,
}
fn charge(current: &mut usize, amount: usize, limit: usize, name: &str) -> Result {
    *current = current
        .checked_add(amount)
        .filter(|total| *total <= limit)
        .ok_or_else(|| Error::resource(format!("Artifact {name} limit exceeded")))?;
    Ok(())
}
impl Budget {
    fn allocation(&mut self, amount: usize) -> Result {
        charge(
            &mut self.allocated,
            amount,
            MAX_ALLOCATED,
            "decoded allocation",
        )
    }
}

struct Reader<'a> {
    version: u16,
    bytes: &'a [u8],
    pos: usize,
    base: usize,
    budget: Rc<RefCell<Budget>>,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            version: VERSION,
            bytes,
            pos: 0,
            base: 0,
            budget: Rc::new(RefCell::new(Budget::default())),
        }
    }
    fn invalid(&self, message: &str) -> Error {
        let mut error = Error::invalid(message);
        error.offset = Some(self.base + self.pos);
        error
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| self.invalid("Truncated artifact"))?;
        let bytes = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.invalid("Invalid Boolean encoding")),
        }
    }
    fn index(&mut self) -> Result<usize> {
        Ok(self.u32()? as usize)
    }
    fn optional_index(&mut self) -> Result<Option<usize>> {
        if self.bool()? {
            Ok(Some(self.index()?))
        } else {
            Ok(None)
        }
    }
    fn string(&mut self) -> Result<String> {
        self.string_with_limit(MAX_STRING_BYTES)
    }
    fn string_with_limit(&mut self, limit: usize) -> Result<String> {
        let count = self.index()?;
        if count > limit {
            return Err(Error::resource("Artifact string length limit exceeded"));
        }
        let bytes = self.take(count)?;
        let text = std::str::from_utf8(bytes).map_err(|_| self.invalid("Invalid UTF-8 string"))?;
        {
            let mut budget = self.budget.borrow_mut();
            charge(
                &mut budget.strings,
                count,
                MAX_TOTAL_STRINGS,
                "total string",
            )?;
            budget.allocation(count)?;
        }
        let mut result = String::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Error::resource("Cannot allocate artifact string"))?;
        result.push_str(text);
        Ok(result)
    }
    fn count(&mut self, limit: usize, min_bytes: usize) -> Result<usize> {
        let count = self.index()?;
        if count > limit {
            return Err(Error::resource("Artifact table count limit exceeded"));
        }
        if count > self.bytes.len().saturating_sub(self.pos) / min_bytes.max(1) {
            return Err(self.invalid("Declared table exceeds remaining section bytes"));
        }
        Ok(count)
    }
    fn vector<T>(
        &mut self,
        limit: usize,
        min_bytes: usize,
        mut item: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<Vec<T>> {
        let count = self.count(limit, min_bytes)?;
        self.vector_count(count, &mut item)
    }
    fn vector_count<T>(
        &mut self,
        count: usize,
        mut item: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<Vec<T>> {
        self.budget.borrow_mut().allocation(
            count
                .checked_mul(size_of::<T>())
                .ok_or_else(|| Error::resource("Artifact allocation size overflow"))?,
        )?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Error::resource("Cannot allocate artifact table"))?;
        for _ in 0..count {
            result.push(item(self)?);
        }
        Ok(result)
    }
    fn indices(&mut self, limit: usize) -> Result<Vec<usize>> {
        let count = self.count(limit, 4)?;
        self.members(count)?;
        self.vector_count(count, Self::index)
    }
    fn members(&self, count: usize) -> Result {
        charge(
            &mut self.budget.borrow_mut().members,
            count,
            MAX_MEMBERS,
            "member",
        )
    }
    fn ty(&mut self, types: &[Type]) -> Result<Type> {
        let index = self.index()?;
        let ty = types
            .get(index)
            .ok_or_else(|| self.invalid("Type reference is not a prior table entry"))?;
        let work = type_nodes(ty)?;
        let mut budget = self.budget.borrow_mut();
        charge(
            &mut budget.type_references,
            1,
            MAX_TYPE_REFERENCES,
            "type reference",
        )?;
        charge(
            &mut budget.type_work,
            work,
            MAX_TYPE_WORK,
            "expanded type work",
        )?;
        // Scalar Type clones are inline; shared payload clones do not allocate.
        Ok(ty.clone())
    }
    fn types(&mut self, types: &[Type]) -> Result<Vec<Type>> {
        self.vector(MAX_MEMBERS, 4, |r| r.ty(types))
    }
    fn locals(&mut self, types: &[Type]) -> Result<Vec<Type>> {
        let count = self.count(MAX_LOCALS, 4)?;
        charge(
            &mut self.budget.borrow_mut().locals,
            count,
            MAX_LOCALS,
            "local slot",
        )?;
        self.vector_count(count, |r| r.ty(types))
    }
    fn effects(&mut self) -> Result<Effects> {
        let bits = self.u8()?;
        if bits & !31 != 0 {
            return Err(self.invalid("Unknown effect bits"));
        }
        let mut effects = Effects::NONE;
        for (bit, flag) in effect_flags() {
            if bits & bit != 0 {
                effects = effects.union(flag);
            }
        }
        Ok(effects)
    }
    fn section(&mut self, id: u8) -> Result<Self> {
        if self.u8()? != id {
            return Err(self.invalid("Missing, reordered, or unknown artifact section"));
        }
        let length = self.index()?;
        let base = self.base + self.pos;
        let bytes = self.take(length)?;
        Ok(Self {
            version: self.version,
            bytes,
            pos: 0,
            base,
            budget: Rc::clone(&self.budget),
        })
    }
    fn finish(&self) -> Result {
        if self.pos != self.bytes.len() {
            Err(self.invalid("Trailing bytes in artifact section"))
        } else {
            Ok(())
        }
    }
    fn span(&mut self, sources: &[SourceFile]) -> Result<Span> {
        let source = SourceId(self.u32()?);
        let start = self.index()?;
        let end = self.index()?;
        let file = sources
            .iter()
            .find(|file| source_id(file) == source)
            .ok_or_else(|| self.invalid("Span references an unknown source"))?;
        file.span(start..end)
            .ok_or_else(|| self.invalid("Span is outside UTF-8 source boundaries"))
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}
impl Writer {
    fn new() -> Self {
        Self::default()
    }
    fn extend(&mut self, bytes: &[u8]) -> Result {
        if bytes.len() > MAX_FILE_BYTES.saturating_sub(self.bytes.len()) {
            return Err(Error::resource("Artifact file length limit exceeded"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| Error::resource("Cannot allocate artifact output"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result {
        self.extend(&[value])
    }
    fn u16(&mut self, value: u16) -> Result {
        self.extend(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result {
        self.extend(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result {
        self.extend(&value.to_le_bytes())
    }
    fn i32(&mut self, value: i32) -> Result {
        self.extend(&value.to_le_bytes())
    }
    fn i64(&mut self, value: i64) -> Result {
        self.extend(&value.to_le_bytes())
    }
    fn bool(&mut self, value: bool) -> Result {
        self.u8(u8::from(value))
    }
    fn index(&mut self, value: usize) -> Result {
        self.u32(u32::try_from(value).map_err(|_| Error::resource("Artifact index exceeds u32"))?)
    }
    fn optional_index(&mut self, value: Option<usize>) -> Result {
        self.bool(value.is_some())?;
        if let Some(value) = value {
            self.index(value)?;
        }
        Ok(())
    }
    fn string(&mut self, value: &str) -> Result {
        self.index(value.len())?;
        self.extend(value.as_bytes())
    }
    fn indices(&mut self, values: &[usize]) -> Result {
        self.index(values.len())?;
        for value in values {
            self.index(*value)?;
        }
        Ok(())
    }
    fn ty(&mut self, table: &TypeTable, value: &Type) -> Result {
        self.index(
            *table
                .indices
                .get(value)
                .ok_or_else(|| Error::invalid("Type was not interned"))?,
        )
    }
    fn types(&mut self, table: &TypeTable, values: &[Type]) -> Result {
        self.index(values.len())?;
        for value in values {
            self.ty(table, value)?;
        }
        Ok(())
    }
    fn effects(&mut self, value: Effects) -> Result {
        let mut bits = 0;
        for (bit, flag) in effect_flags() {
            if value.contains(flag) {
                bits |= bit;
            }
        }
        self.u8(bits)
    }
    fn section(&mut self, id: u8, write: impl FnOnce(&mut Self) -> Result) -> Result {
        self.u8(id)?;
        let length_at = self.bytes.len();
        self.u32(0)?;
        let start = self.bytes.len();
        write(self)?;
        let length = u32::try_from(self.bytes.len() - start)
            .map_err(|_| Error::resource("Section exceeds u32"))?;
        self.bytes[length_at..length_at + 4].copy_from_slice(&length.to_le_bytes());
        Ok(())
    }
    fn span(&mut self, span: Span) -> Result {
        self.u32(span.source().0)?;
        self.index(span.range().start)?;
        self.index(span.range().end)
    }
}
fn effect_flags() -> [(u8, Effects); 5] {
    [
        (1, Effects::READ),
        (2, Effects::WRITE),
        (4, Effects::OBSERVE),
        (8, Effects::IO),
        (16, Effects::ABORT),
    ]
}
fn source_id(source: &SourceFile) -> SourceId {
    source.span(0..0).unwrap().source()
}

#[derive(Default)]
struct TypeTable {
    types: Vec<Type>,
    indices: HashMap<Type, usize>,
}
impl TypeTable {
    fn intern(&mut self, ty: &Type) -> Result<usize> {
        type_nodes(ty)?;
        if let Some(index) = self.indices.get(ty) {
            return Ok(*index);
        }
        match ty {
            Type::Function(signature) => {
                for parameter in &signature.parameters {
                    self.intern(parameter)?;
                }
                self.intern(&signature.result)?;
            }
            Type::Baddie(payload)
            | Type::Array(payload)
            | Type::FixedArray(payload, _)
            | Type::Lazy(payload, _) => {
                self.intern(payload)?;
            }
            Type::Result(payloads) => {
                self.intern(&payloads.0)?;
                self.intern(&payloads.1)?;
            }
            _ => {}
        }
        if self.types.len() >= MAX_TYPES {
            return Err(Error::resource("Artifact type count limit exceeded"));
        }
        let index = self.types.len();
        self.types.push(ty.clone());
        self.indices.insert(ty.clone(), index);
        Ok(index)
    }
}
fn type_nodes(ty: &Type) -> Result<usize> {
    let mut pending = vec![(ty, 0)];
    let mut count = 0;
    let mut work = 0_usize;
    while let Some((ty, depth)) = pending.pop() {
        count += 1;
        work += 1;
        if count > 4096 || depth > 128 {
            return Err(Error::resource("Artifact type complexity limit exceeded"));
        }
        match ty {
            Type::Class(_, name) | Type::Role(_, name) => {
                work = work
                    .checked_add(name.len())
                    .filter(|work| *work <= MAX_TYPE_WORK)
                    .ok_or_else(|| {
                        Error::resource("Artifact nominal type name work limit exceeded")
                    })?;
            }
            Type::Function(signature) => {
                if signature.parameters.len() + 1 > 4096_usize.saturating_sub(count + pending.len())
                {
                    return Err(Error::resource(
                        "Artifact function type complexity limit exceeded",
                    ));
                }
                pending.push((&signature.result, depth + 1));
                pending.extend(signature.parameters.iter().map(|ty| (ty, depth + 1)));
            }
            Type::Baddie(payload)
            | Type::Array(payload)
            | Type::FixedArray(payload, _)
            | Type::Lazy(payload, _) => pending.push((payload, depth + 1)),
            Type::Result(payloads) => {
                pending.push((&payloads.0, depth + 1));
                pending.push((&payloads.1, depth + 1));
            }
            _ => {}
        }
    }
    Ok(work)
}

fn collect_types(module: &Module) -> Result<TypeTable> {
    let mut table = TypeTable::default();
    for role in &module.roles {
        for method in &role.methods {
            for ty in &method.parameters {
                table.intern(ty)?;
            }
            table.intern(&method.result)?;
        }
    }
    for class in &module.classes {
        for field in &class.fields {
            table.intern(&field.ty)?;
        }
    }
    for extension in &module.extensions {
        table.intern(&extension.receiver)?;
    }
    for function in &module.functions {
        for ty in function
            .parameters
            .iter()
            .chain(&function.captures)
            .chain(&function.locals)
            .chain(std::iter::once(&function.result))
        {
            table.intern(ty)?;
        }
        for instruction in &function.code {
            codec::collect_op_types(&mut table, &instruction.op)?;
        }
    }
    Ok(table)
}

// Type tags: 0 Mf, 1 Unc, 2 Geeked, 3 Verdict, 4 Text, 5 Unit,
// 6 Function, 7 Baddie, 8 Result, 9 Array, 10 FixedArray,
// 11 ConversionError, 12 Class, 13 Role, 14 Lazy. Child references precede parents.
fn write_type(w: &mut Writer, table: &TypeTable, ty: &Type) -> Result {
    let tag = match ty {
        Type::Mf => 0,
        Type::Unc => 1,
        Type::Geeked => 2,
        Type::Verdict => 3,
        Type::Text => 4,
        Type::Unit => 5,
        Type::Function(_) => 6,
        Type::Baddie(_) => 7,
        Type::Result(_) => 8,
        Type::Array(_) => 9,
        Type::FixedArray(..) => 10,
        Type::ConversionError => 11,
        Type::Class(..) => 12,
        Type::Role(..) => 13,
        Type::Lazy(..) => 14,
    };
    w.u8(tag)?;
    match ty {
        Type::Function(signature) => {
            w.types(table, &signature.parameters)?;
            w.ty(table, &signature.result)?;
            w.effects(signature.effects)?;
        }
        Type::Baddie(payload) | Type::Array(payload) => w.ty(table, payload)?,
        Type::Result(payloads) => {
            w.ty(table, &payloads.0)?;
            w.ty(table, &payloads.1)?;
        }
        Type::FixedArray(element, length) => {
            w.ty(table, element)?;
            w.index(*length)?;
        }
        Type::Class(index, name) | Type::Role(index, name) => {
            w.index(*index)?;
            w.string(name)?;
        }
        Type::Lazy(result, effects) => {
            w.ty(table, result)?;
            w.effects(*effects)?;
        }
        _ => {}
    }
    Ok(())
}
fn read_type(r: &mut Reader<'_>, types: &[Type]) -> Result<Type> {
    r.budget.borrow_mut().allocation(128)?;
    let ty = match r.u8()? {
        0 => Type::Mf,
        1 => Type::Unc,
        2 => Type::Geeked,
        3 => Type::Verdict,
        4 => Type::Text,
        5 => Type::Unit,
        6 => {
            let parameters = r.vector(4096, 4, |r| r.ty(types))?;
            let result = r.ty(types)?;
            let effects = r.effects()?;
            Type::function(parameters, result, effects)
        }
        7 => Type::option(r.ty(types)?),
        8 => {
            let success = r.ty(types)?;
            Type::result(success, r.ty(types)?)
        }
        9 => Type::array(r.ty(types)?),
        10 => {
            let element = r.ty(types)?;
            Type::fixed_array(element, r.index()?)
        }
        11 => Type::ConversionError,
        12 | 13 => {
            let tag = r.bytes[r.pos - 1];
            let index = r.index()?;
            let name = r.string()?;
            r.budget.borrow_mut().allocation(name.len())?;
            if tag == 12 {
                Type::class(index, name.as_str())
            } else {
                Type::role(index, name)
            }
        }
        14 => {
            let result = r.ty(types)?;
            Type::lazy(result, r.effects()?)
        }
        _ => return Err(r.invalid("Unknown type tag")),
    };
    let work = type_nodes(&ty)?;
    charge(
        &mut r.budget.borrow_mut().type_work,
        work,
        MAX_TYPE_WORK,
        "expanded type work",
    )?;
    if !ty.is_well_formed() {
        return Err(r.invalid("Malformed artifact type"));
    }
    Ok(ty)
}

/// Encode a verified module deterministically, embedding its diagnostic sources.
/// The resulting bytes must also satisfy the loader's resource policy.
pub fn encode(module: &VerifiedModule, sources: &[SourceFile]) -> Result<Vec<u8>> {
    let module = module.module();
    let types = collect_types(module)?;
    let mut w = Writer::new();
    w.extend(MAGIC)?;
    w.u16(VERSION)?;
    w.u16(0)?;
    w.u32(0)?;
    w.section(1, |w| {
        w.index(sources.len())?;
        for source in sources {
            w.u32(source_id(source).0)?;
            w.string(source.name())?;
            w.string(source.text())?;
        }
        Ok(())
    })?;
    w.section(2, |w| {
        w.index(types.types.len())?;
        for ty in &types.types {
            write_type(w, &types, ty)?;
        }
        Ok(())
    })?;
    w.section(3, |w| {
        w.index(module.roles.len())?;
        for role in &module.roles {
            w.string(&role.name)?;
            w.index(role.methods.len())?;
            for method in &role.methods {
                w.string(&method.name)?;
                w.types(&types, &method.parameters)?;
                w.ty(&types, &method.result)?;
                w.effects(method.effects)?;
            }
        }
        Ok(())
    })?;
    w.section(4, |w| {
        w.index(module.classes.len())?;
        for class in &module.classes {
            w.string(&class.name)?;
            w.index(class.constructor)?;
            w.index(class.fields.len())?;
            for field in &class.fields {
                w.string(&field.name)?;
                w.ty(&types, &field.ty)?;
                w.bool(field.public)?;
            }
            w.index(class.methods.len())?;
            for method in &class.methods {
                w.string(&method.name)?;
                w.index(method.function)?;
                w.bool(method.public)?;
            }
            w.index(class.implementations.len())?;
            for implementation in &class.implementations {
                w.index(implementation.role)?;
                w.indices(&implementation.methods)?;
            }
        }
        Ok(())
    })?;
    w.section(5, |w| {
        w.index(module.extensions.len())?;
        for extension in &module.extensions {
            w.ty(&types, &extension.receiver)?;
            w.string(&extension.name)?;
            w.index(extension.function)?;
        }
        Ok(())
    })?;
    w.section(6, |w| {
        w.index(module.functions.len())?;
        for function in &module.functions {
            w.bool(function.scenario.is_some())?;
            if let Some(context) = function.scenario {
                w.index(context.entry)?;
                w.index(context.class)?;
                w.optional_index(context.origin)?;
            }
            w.optional_index(function.owner)?;
            w.optional_index(function.constructor)?;
            w.string(&function.name)?;
            w.types(&types, &function.parameters)?;
            w.types(&types, &function.captures)?;
            w.ty(&types, &function.result)?;
            w.effects(function.effects)?;
            w.types(&types, &function.locals)?;
            w.span(function.span)?;
            w.index(function.code.len())?;
            for instruction in &function.code {
                codec::write_op(w, &types, &instruction.op)?;
                w.span(instruction.span)?;
            }
        }
        Ok(())
    })?;
    w.section(7, |w| w.index(module.entry))?;
    let length = u32::try_from(w.bytes.len() - 16)
        .map_err(|_| Error::resource("Artifact payload exceeds u32"))?;
    w.bytes[12..16].copy_from_slice(&length.to_le_bytes());
    // One policy applies in both directions, including aggregate budgets and spans.
    decode(&w.bytes)?;
    Ok(w.bytes)
}

/// Decode untrusted bytes and independently verify the reconstructed module.
pub fn decode(bytes: &[u8]) -> Result<Artifact> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Error::resource("Artifact file length limit exceeded"));
    }
    let mut r = Reader::new(bytes);
    if r.take(8)? != MAGIC {
        return Err(r.invalid("Invalid artifact magic"));
    }
    let version = r.u16()?;
    if !SUPPORTED_VERSIONS.contains(&version) {
        return Err(r.invalid("Unsupported artifact version"));
    }
    r.version = version;
    if r.u16()? != 0 {
        return Err(r.invalid("Unsupported artifact flags"));
    }
    if r.index()? != bytes.len().saturating_sub(16) {
        return Err(r.invalid("Artifact payload length does not match file"));
    }
    let mut s = r.section(1)?;
    let mut ids = Vec::new();
    let sources = s.vector(MAX_SOURCES, 12, |s| {
        let id = SourceId(s.u32()?);
        if ids.contains(&id) {
            return Err(s.invalid("Duplicate embedded source ID"));
        }
        ids.push(id);
        let name = s.string()?;
        let text = s.string_with_limit(MAX_SOURCE_BYTES)?;
        let lines = 1 + text.bytes().filter(|byte| *byte == b'\n').count();
        {
            let mut budget = s.budget.borrow_mut();
            charge(
                &mut budget.source_bytes,
                text.len(),
                MAX_SOURCE_BYTES,
                "source text",
            )?;
            charge(
                &mut budget.source_lines,
                lines,
                MAX_SOURCE_LINES,
                "source line",
            )?;
            // SourceFile builds its line index incrementally; charge twice the length.
            budget.allocation(lines * size_of::<usize>() * 2)?;
        }
        Ok(SourceFile::new(id, name, text))
    })?;
    s.finish()?;
    let mut s = r.section(2)?;
    let count = s.count(MAX_TYPES, 1)?;
    s.budget
        .borrow_mut()
        .allocation(count * size_of::<Type>())?;
    let mut types = Vec::new();
    types
        .try_reserve_exact(count)
        .map_err(|_| Error::resource("Cannot allocate artifact type table"))?;
    for _ in 0..count {
        let ty = read_type(&mut s, &types)?;
        types.push(ty);
    }
    s.finish()?;
    let mut s = r.section(3)?;
    let roles = s.vector(MAX_DECLARATIONS, 8, |s| {
        let name = s.string()?;
        let methods = s.vector(MAX_MEMBERS, 13, |s| {
            Ok(RoleMethod {
                name: s.string()?,
                parameters: s.types(&types)?,
                result: s.ty(&types)?,
                effects: s.effects()?,
            })
        })?;
        s.members(methods.len())?;
        Ok(Role { name, methods })
    })?;
    s.finish()?;
    let mut s = r.section(4)?;
    let classes = s.vector(MAX_DECLARATIONS, 20, |s| {
        let name = s.string()?;
        let constructor = s.index()?;
        let fields = s.vector(MAX_MEMBERS, 9, |s| {
            Ok(Field {
                name: s.string()?,
                ty: s.ty(&types)?,
                public: s.bool()?,
            })
        })?;
        s.members(fields.len())?;
        let methods = s.vector(MAX_MEMBERS, 9, |s| {
            Ok(Method {
                name: s.string()?,
                function: s.index()?,
                public: s.bool()?,
            })
        })?;
        s.members(methods.len())?;
        let implementations = s.vector(MAX_MEMBERS, 8, |s| {
            Ok(Implementation {
                role: s.index()?,
                methods: s.indices(MAX_MEMBERS)?,
            })
        })?;
        s.members(implementations.len())?;
        Ok(Class {
            implementations,
            name,
            fields,
            constructor,
            methods,
        })
    })?;
    s.finish()?;
    let mut s = r.section(5)?;
    let extensions = s.vector(MAX_DECLARATIONS, 12, |s| {
        Ok(Extension {
            receiver: s.ty(&types)?,
            name: s.string()?,
            function: s.index()?,
        })
    })?;
    s.finish()?;
    let mut s = r.section(6)?;
    let functions = s.vector(MAX_DECLARATIONS, 37, |s| {
        let scenario = if s.bool()? {
            Some(ScenarioContext {
                entry: s.index()?,
                class: s.index()?,
                origin: s.optional_index()?,
            })
        } else {
            None
        };
        let owner = s.optional_index()?;
        let constructor = s.optional_index()?;
        let name = s.string()?;
        let parameters = s.types(&types)?;
        let captures = s.types(&types)?;
        let result = s.ty(&types)?;
        let effects = s.effects()?;
        let locals = s.locals(&types)?;
        let span = s.span(&sources)?;
        let count = s.count(MAX_INSTRUCTIONS, 13)?;
        charge(
            &mut s.budget.borrow_mut().instructions,
            count,
            MAX_INSTRUCTIONS,
            "instruction",
        )?;
        let code = s.vector_count(count, |s| {
            Ok(Instruction {
                op: codec::read_op(s, &types)?,
                span: s.span(&sources)?,
            })
        })?;
        Ok(Function {
            scenario,
            owner,
            constructor,
            name,
            parameters,
            captures,
            result,
            effects,
            locals,
            code,
            span,
        })
    })?;
    s.finish()?;
    let mut s = r.section(7)?;
    let entry = s.index()?;
    s.finish()?;
    r.finish()?;
    let module = larp_bytecode::verify(Module {
        roles,
        extensions,
        classes,
        functions,
        entry,
    })
    .map_err(|error| Error {
        kind: if error.kind == larp_bytecode::VerifyErrorKind::ResourceLimit {
            ErrorKind::ResourceLimit
        } else {
            ErrorKind::InvalidBytecode
        },
        offset: None,
        message: format!(
            "Invalid bytecode in function {}: {}",
            error.function, error.message
        ),
        verification: Some(error),
    })?;
    Ok(Artifact { module, sources })
}

/// Read at most the file limit plus one sentinel byte, even from an unbounded stream.
pub fn read(reader: &mut impl Read) -> Result<Artifact> {
    let mut bytes = Vec::new();
    let mut limited = reader.take((MAX_FILE_BYTES + 1) as u64);
    let mut chunk = [0_u8; 8192];
    loop {
        let count = match limited.read(&mut chunk) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("Cannot read artifact: {error}"),
                ));
            }
        };
        if count == 0 {
            break;
        }
        if count > MAX_FILE_BYTES.saturating_sub(bytes.len()) {
            return Err(Error::resource("Artifact file length limit exceeded"));
        }
        bytes
            .try_reserve(count)
            .map_err(|_| Error::resource("Cannot allocate artifact input"))?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larp_bytecode::Op;
    use larp_types::Scalar;

    fn sample() -> (VerifiedModule, Vec<SourceFile>) {
        let source = SourceFile::new(SourceId(777), "fixture.larp", "🦀");
        let span = source.span(0..4).unwrap();
        let module = larp_bytecode::verify(Module {
            roles: vec![],
            extensions: vec![],
            classes: vec![],
            entry: 0,
            functions: vec![Function {
                scenario: None,
                owner: None,
                constructor: None,
                name: "main".into(),
                parameters: vec![],
                captures: vec![],
                result: Type::Unit,
                effects: Effects::NONE,
                locals: vec![],
                span,
                code: vec![
                    Instruction {
                        op: Op::Constant(Scalar::Unit),
                        span,
                    },
                    Instruction {
                        op: Op::Return,
                        span,
                    },
                ],
            }],
        })
        .unwrap();
        (module, vec![source])
    }
    fn artifact() -> Vec<u8> {
        let (module, sources) = sample();
        encode(&module, &sources).unwrap()
    }
    fn frame(sections: impl FnOnce(&mut Writer) -> Result) -> Vec<u8> {
        let mut w = Writer::new();
        w.extend(MAGIC).unwrap();
        w.u16(VERSION).unwrap();
        w.u16(0).unwrap();
        w.u32(0).unwrap();
        sections(&mut w).unwrap();
        let count = (w.bytes.len() - 16) as u32;
        w.bytes[12..16].copy_from_slice(&count.to_le_bytes());
        w.bytes
    }
    fn sections(bytes: &[u8]) -> Vec<(u8, usize, usize)> {
        let mut r = Reader::new(bytes);
        r.take(16).unwrap();
        let mut result = vec![];
        while r.pos < bytes.len() {
            let id = r.u8().unwrap();
            let size = r.index().unwrap();
            result.push((id, r.pos, size));
            r.take(size).unwrap();
        }
        result
    }

    #[test]
    fn deterministic_roundtrip_embeds_source_identity_and_utf8_spans() {
        let (module, sources) = sample();
        let bytes = encode(&module, &sources).unwrap();
        assert_eq!(&bytes[..8], MAGIC);
        let artifact = decode(&bytes).unwrap();
        assert_eq!(artifact.module.disassemble(), module.disassemble());
        assert_eq!(artifact.sources[0].text(), "🦀");
        let span = artifact.module.module().functions[0].span;
        assert_eq!(artifact.source(span).unwrap().slice(span), Some("🦀"));
        assert_eq!(encode(&artifact.module, &artifact.sources).unwrap(), bytes);
        assert_eq!(encode(&module, &sources).unwrap(), bytes);
        assert_eq!(
            read(&mut bytes.as_slice()).unwrap().module.disassemble(),
            module.disassemble()
        );
    }

    #[test]
    fn all_truncations_unknown_headers_section_order_and_trailing_bytes_fail() {
        let bytes = artifact();
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        for (index, value) in [(0, b'X'), (8, 3), (10, 1), (16, 7)] {
            let mut changed = bytes.clone();
            changed[index] = value;
            assert_eq!(
                decode(&changed).unwrap_err().kind(),
                ErrorKind::InvalidFormat
            );
        }
        let mut changed = bytes.clone();
        changed.push(0);
        assert!(decode(&changed).is_err());
        let length = changed.len() - 16;
        changed[12..16].copy_from_slice(&(length as u32).to_le_bytes());
        assert!(decode(&changed).is_err());
        // Even correctly framed section contents must be consumed exactly.
        let mut changed = bytes;
        let (_, pos, size) = sections(&changed)[6];
        changed.insert(pos + size, 0);
        changed[pos - 4..pos].copy_from_slice(&((size + 1) as u32).to_le_bytes());
        let length = changed.len() - 16;
        changed[12..16].copy_from_slice(&(length as u32).to_le_bytes());
        assert!(decode(&changed).is_err());
    }

    #[test]
    fn valid_framing_cannot_bypass_bytecode_verification() {
        let mut bytes = artifact();
        let end = bytes.len();
        bytes[end - 4..].copy_from_slice(&1_u32.to_le_bytes());
        let error = decode(&bytes).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidBytecode);
        assert!(error.verification().unwrap().message.contains("Entry"));
    }

    #[test]
    fn malformed_source_ids_and_utf8_span_boundaries_fail_before_verification() {
        let bytes = artifact();
        let (_, start, size) = sections(&bytes)[5];
        let pattern = [
            777_u32.to_le_bytes(),
            0_u32.to_le_bytes(),
            4_u32.to_le_bytes(),
        ]
        .concat();
        let position = start
            + bytes[start..start + size]
                .windows(12)
                .position(|part| part == pattern)
                .unwrap();
        for (offset, value) in [(0, 778_u32), (4, 1), (8, 5)] {
            let mut changed = bytes.clone();
            changed[position + offset..position + offset + 4].copy_from_slice(&value.to_le_bytes());
            let error = decode(&changed).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::InvalidFormat);
            assert!(error.to_string().contains("Span"));
            assert!(error.offset().is_some());
        }
        let (module, sources) = sample();
        assert!(encode(&module, &[]).is_err());
        let duplicate = SourceFile::new(SourceId(777), "duplicate", "");
        assert!(encode(&module, &[sources.into_iter().next().unwrap(), duplicate]).is_err());
    }

    #[test]
    fn hostile_counts_strings_type_references_effects_and_depth_are_bounded() {
        let count = frame(|w| w.section(1, |w| w.u32(u32::MAX)));
        assert_eq!(decode(&count).unwrap_err().kind(), ErrorKind::ResourceLimit);
        let forward = frame(|w| {
            w.section(1, |w| w.index(0))?;
            w.section(2, |w| {
                w.index(1)?;
                w.u8(9)?;
                w.index(0)
            })
        });
        assert_eq!(
            decode(&forward).unwrap_err().kind(),
            ErrorKind::InvalidFormat
        );
        let exponential = frame(|w| {
            w.section(1, |w| w.index(0))?;
            w.section(2, |w| {
                w.index(32)?;
                w.u8(0)?;
                for index in 0..31 {
                    w.u8(8)?;
                    w.index(index)?;
                    w.index(index)?;
                }
                Ok(())
            })
        });
        assert_eq!(
            decode(&exponential).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
        let deep = frame(|w| {
            w.section(1, |w| w.index(0))?;
            w.section(2, |w| {
                w.index(131)?;
                w.u8(0)?;
                for index in 0..130 {
                    w.u8(7)?;
                    w.index(index)?;
                }
                Ok(())
            })
        });
        assert_eq!(decode(&deep).unwrap_err().kind(), ErrorKind::ResourceLimit);
        let effects = frame(|w| {
            w.section(1, |w| w.index(0))?;
            w.section(2, |w| {
                w.index(2)?;
                w.u8(0)?;
                w.u8(14)?;
                w.index(0)?;
                w.u8(32)
            })
        });
        assert_eq!(
            decode(&effects).unwrap_err().kind(),
            ErrorKind::InvalidFormat
        );
        let string = frame(|w| {
            w.section(1, |w| {
                w.index(1)?;
                w.index(1)?;
                w.u32(u32::MAX)?;
                w.u32(0)
            })
        });
        assert_eq!(
            decode(&string).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
    }

    #[test]
    fn type_interning_rejects_expanded_dags_before_structural_hashing() {
        let mut ty = Type::Mf;
        for _ in 0..30 {
            ty = Type::result(ty.clone(), ty);
        }
        assert_eq!(
            TypeTable::default().intern(&ty).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
    }

    #[test]
    fn total_type_work_and_references_are_shared_between_sections() {
        let bytes = [0, 0, 0, 0];
        let mut reader = Reader::new(&bytes);
        reader.budget.borrow_mut().type_work = MAX_TYPE_WORK;
        assert_eq!(
            reader.ty(&[Type::Mf]).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
        let mut reader = Reader::new(&bytes);
        reader.budget.borrow_mut().type_references = MAX_TYPE_REFERENCES;
        assert_eq!(
            reader.ty(&[Type::Mf]).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
    }

    #[test]
    fn source_line_budgets_precede_source_file_line_index_allocation() {
        let (module, _) = sample();
        let source = SourceFile::new(SourceId(777), "many-lines", "\n".repeat(MAX_SOURCE_LINES));
        assert_eq!(
            encode(&module, &[source]).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
    }

    #[test]
    fn input_reader_stops_after_one_oversize_sentinel_and_reports_io_failures() {
        struct Infinite {
            consumed: usize,
        }
        impl Read for Infinite {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.consumed += buffer.len();
                buffer.fill(0);
                Ok(buffer.len())
            }
        }
        let mut input = Infinite { consumed: 0 };
        assert_eq!(
            read(&mut input).unwrap_err().kind(),
            ErrorKind::ResourceLimit
        );
        assert_eq!(input.consumed, MAX_FILE_BYTES + 1);
        struct Failure;
        impl Read for Failure {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("injected read failure"))
            }
        }
        assert_eq!(read(&mut Failure).unwrap_err().kind(), ErrorKind::Io);
    }
    #[test]
    fn shared_nominal_name_bytes_count_toward_expanded_type_work() {
        let bytes = frame(|w| {
            w.section(1, |w| w.index(0))?;
            w.section(2, |w| {
                w.index(3)?;
                w.u8(5)?;
                w.u8(12)?;
                w.index(0)?;
                w.string(&"N".repeat(65_536))?;
                w.u8(6)?;
                w.index(64)?;
                for _ in 0..64 {
                    w.index(1)?;
                }
                w.index(0)?;
                w.u8(0)
            })
        });
        let error = decode(&bytes).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ResourceLimit);
        assert!(error.to_string().contains("type work"));
    }

    #[test]
    fn input_reader_retries_interrupted_reads() {
        struct Interrupted {
            first: bool,
            bytes: std::io::Cursor<Vec<u8>>,
        }
        impl Read for Interrupted {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                if self.first {
                    self.first = false;
                    Err(std::io::ErrorKind::Interrupted.into())
                } else {
                    self.bytes.read(bytes)
                }
            }
        }
        let mut input = Interrupted {
            first: true,
            bytes: std::io::Cursor::new(artifact()),
        };
        assert!(read(&mut input).is_ok());
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;
    use larp_bytecode::Op;
    use larp_types::{Intrinsic, Scalar};

    #[test]
    fn version_one_modules_still_load_but_cannot_claim_version_two_host_intrinsics() {
        let source = SourceFile::new(SourceId(0), "version.larp", "");
        let span = source.span(0..0).unwrap();
        let function = Function {
            scenario: None,
            owner: None,
            constructor: None,
            name: "main".into(),
            parameters: vec![],
            captures: vec![],
            result: Type::Unit,
            effects: Effects::IO,
            locals: vec![Type::array(Type::Text)],
            span,
            code: vec![
                Instruction {
                    op: Op::Constant(Scalar::Unit),
                    span,
                },
                Instruction {
                    op: Op::Return,
                    span,
                },
            ],
        };
        let module = |function| {
            larp_bytecode::verify(Module {
                roles: vec![],
                classes: vec![],
                extensions: vec![],
                functions: vec![function],
                entry: 0,
            })
            .unwrap()
        };
        let mut old = encode(&module(function.clone()), &[source]).unwrap();
        assert_eq!(u16::from_le_bytes(old[8..10].try_into().unwrap()), VERSION);
        old[8..10].copy_from_slice(&1u16.to_le_bytes());
        decode(&old).unwrap();
        for intrinsic in [Intrinsic::Arguments, Intrinsic::ReadLine] {
            let mut function = function.clone();
            function.locals[0] = intrinsic.signature().unwrap().result;
            function.code.splice(
                0..0,
                [
                    Instruction {
                        op: Op::Intrinsic(intrinsic),
                        span,
                    },
                    Instruction {
                        op: Op::Store(0),
                        span,
                    },
                ],
            );
            let source = SourceFile::new(SourceId(0), "version.larp", "");
            let mut bytes = encode(&module(function), &[source]).unwrap();
            decode(&bytes).unwrap();
            bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
            assert_eq!(decode(&bytes).unwrap_err().kind(), ErrorKind::InvalidFormat);
        }
    }
}
