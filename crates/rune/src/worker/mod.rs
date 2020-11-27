//! Worker used by compiler.

use crate::ast;
use crate::collections::HashMap;
use crate::indexing::{Index as _, IndexScopes, Indexer};
use crate::query::Query;
use crate::shared::{Consts, Gen, Items};
use crate::{
    CompileVisitor, Error, Errors, Options, SourceLoader, Sources, Storage, UnitBuilder, Warnings,
};
use runestick::{Context, Item, SourceId, Span};
use std::collections::VecDeque;

mod import;
mod task;
mod wildcard_import;

pub(crate) use self::import::Import;
pub(crate) use self::task::{LoadFileKind, Task};
pub(crate) use self::wildcard_import::WildcardImport;

pub(crate) struct Worker<'a> {
    context: &'a Context,
    pub(crate) sources: &'a mut Sources,
    options: &'a Options,
    pub(crate) errors: &'a mut Errors,
    pub(crate) warnings: &'a mut Warnings,
    pub(crate) visitor: &'a mut dyn CompileVisitor,
    pub(crate) source_loader: &'a mut dyn SourceLoader,
    /// Constants storage.
    pub(crate) consts: Consts,
    /// Worker queue.
    pub(crate) queue: VecDeque<Task>,
    /// Query engine.
    pub(crate) query: Query,
    /// Macro storage.
    pub(crate) storage: Storage,
    /// Id generator.
    pub(crate) gen: Gen,
    /// Files that have been loaded.
    pub(crate) loaded: HashMap<Item, (SourceId, Span)>,
}

impl<'a> Worker<'a> {
    /// Construct a new worker.
    pub(crate) fn new(
        context: &'a Context,
        sources: &'a mut Sources,
        options: &'a Options,
        unit: UnitBuilder,
        consts: Consts,
        errors: &'a mut Errors,
        warnings: &'a mut Warnings,
        visitor: &'a mut dyn CompileVisitor,
        source_loader: &'a mut dyn SourceLoader,
        storage: Storage,
        gen: Gen,
    ) -> Self {
        Self {
            context,
            sources,
            options,
            errors,
            warnings,
            visitor,
            source_loader,
            consts: consts.clone(),
            queue: VecDeque::new(),
            query: Query::new(storage.clone(), unit, consts, gen.clone()),
            storage,
            gen,
            loaded: HashMap::new(),
        }
    }

    /// Run the worker until the task queue is empty.
    pub(crate) fn run(&mut self) {
        // NB: defer wildcard expansion until all other imports have been
        // indexed.
        let mut wildcard_imports = Vec::new();

        while let Some(task) = self.queue.pop_front() {
            match task {
                Task::LoadFile {
                    kind,
                    source_id,
                    mod_item,
                } => {
                    log::trace!("load file: {}", mod_item.item);

                    let source = match self.sources.get(source_id).cloned() {
                        Some(source) => source,
                        None => {
                            self.errors
                                .push(Error::internal(source_id, "missing queued source by id"));

                            continue;
                        }
                    };

                    let mut file = match crate::parse_all::<ast::File>(source.as_str()) {
                        Ok(file) => file,
                        Err(error) => {
                            self.errors.push(Error::new(source_id, error));

                            continue;
                        }
                    };

                    let root = match kind {
                        LoadFileKind::Root => source.path().map(ToOwned::to_owned),
                        LoadFileKind::Module { root } => root,
                    };

                    log::trace!("index: {}", mod_item.item);
                    let items = Items::new(mod_item.item.clone(), self.gen.clone());

                    let mut indexer = Indexer {
                        root,
                        storage: self.query.storage(),
                        loaded: &mut self.loaded,
                        consts: self.consts.clone(),
                        query: self.query.clone(),
                        queue: &mut self.queue,
                        sources: self.sources,
                        context: self.context,
                        options: self.options,
                        source_id,
                        source,
                        warnings: self.warnings,
                        items,
                        scopes: IndexScopes::new(),
                        mod_item,
                        impl_item: Default::default(),
                        visitor: self.visitor,
                        source_loader: self.source_loader,
                    };

                    if let Err(error) = file.index(&mut indexer) {
                        self.errors.push(Error::new(source_id, error));
                    }
                }
                Task::ExpandImport(import) => {
                    let source_id = import.source_id;
                    let queue = &mut self.queue;

                    let result =
                        import.process(&self.context, &self.storage, &self.query, &mut |task| {
                            queue.push_back(task);
                        });

                    if let Err(error) = result {
                        self.errors.push(Error::new(source_id, error));
                    }
                }
                Task::ExpandWildcardImport(wildcard_import) => {
                    wildcard_imports.push(wildcard_import);
                }
            }
        }

        for wildcard_import in wildcard_imports {
            let source_id = wildcard_import.source_id;

            if let Err(error) = wildcard_import.process_local(&self.query) {
                self.errors.push(Error::new(source_id, error));
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ImportKind {
    /// The import is in-place.
    Local,
    /// The import is deferred.
    Global,
}
