use std::{any::Any, sync::Arc};

use async_trait::async_trait;
use bigdecimal::ToPrimitive;

use datafusion::{
    arrow::{
        array::{Array, ArrayRef, BooleanBuilder, Int32Builder, StringBuilder, UInt32Builder},
        datatypes::{DataType, Field, Schema, SchemaRef},
        record_batch::RecordBatch,
    },
    datasource::{datasource::TableProviderFilterPushDown, TableProvider, TableType},
    error::DataFusionError,
    logical_plan::Expr,
    physical_plan::{memory::MemoryExec, ExecutionPlan},
};
use pg_srv::PgTypeId;

use crate::{
    compile::engine::information_schema::postgres::{
        PG_NAMESPACE_CATALOG_OID, PG_NAMESPACE_PUBLIC_OID,
    },
    transport::CubeMetaTable,
};

// See https://github.com/postgres/postgres/blob/REL_16_4/src/include/catalog/pg_class.h#L32
pub const PG_CLASS_CLASS_OID: u32 = 1259;
const PG_CLASS_ROWTYPE_OID: u32 = PgTypeId::PGCLASS.to_type().oid;

struct PgClass {
    oid: u32,
    relname: String,
    relnamespace: u32,
    reltype: u32,
    relam: u32,
    relfilenode: u32,
    reltoastrelid: u32,
    relisshared: bool,
    relkind: String,
    relnatts: i32,
    relhasrules: bool,
    relreplident: String,
    relfrozenxid: i32,
    relminmxid: i32,
}

struct PgCatalogClassBuilder {
    oid: UInt32Builder,
    relname: StringBuilder,
    relnamespace: UInt32Builder,
    reltype: UInt32Builder,
    reloftype: UInt32Builder,
    relowner: UInt32Builder,
    relam: UInt32Builder,
    relfilenode: UInt32Builder,
    reltablespace: UInt32Builder,
    relpages: Int32Builder,
    reltuples: Int32Builder,
    relallvisible: Int32Builder,
    reltoastrelid: UInt32Builder,
    relhasindex: BooleanBuilder,
    relisshared: BooleanBuilder,
    relpersistence: StringBuilder,
    relkind: StringBuilder,
    relnatts: Int32Builder,
    relchecks: Int32Builder,
    relhasrules: BooleanBuilder,
    relhastriggers: BooleanBuilder,
    relhassubclass: BooleanBuilder,
    relrowsecurity: BooleanBuilder,
    relforcerowsecurity: BooleanBuilder,
    relispopulated: BooleanBuilder,
    relreplident: StringBuilder,
    relispartition: BooleanBuilder,
    relrewrite: UInt32Builder,
    relfrozenxid: Int32Builder,
    relminmxid: Int32Builder,
    relacl: StringBuilder,
    reloptions: StringBuilder,
    relpartbound: StringBuilder,
    xmin: UInt32Builder,
    // This column was removed after PostgreSQL 12, but it's required to support Tableau Desktop with ODBC
    // True if we generate an OID for each row of the relation
    relhasoids: BooleanBuilder,
}

impl PgCatalogClassBuilder {
    fn new() -> Self {
        let capacity = 10;

        Self {
            oid: UInt32Builder::new(capacity),
            relname: StringBuilder::new(capacity),
            relnamespace: UInt32Builder::new(capacity),
            reltype: UInt32Builder::new(capacity),
            reloftype: UInt32Builder::new(capacity),
            relowner: UInt32Builder::new(capacity),
            relam: UInt32Builder::new(capacity),
            relfilenode: UInt32Builder::new(capacity),
            reltablespace: UInt32Builder::new(capacity),
            relpages: Int32Builder::new(capacity),
            reltuples: Int32Builder::new(capacity),
            relallvisible: Int32Builder::new(capacity),
            reltoastrelid: UInt32Builder::new(capacity),
            relhasindex: BooleanBuilder::new(capacity),
            relisshared: BooleanBuilder::new(capacity),
            relpersistence: StringBuilder::new(capacity),
            relkind: StringBuilder::new(capacity),
            relnatts: Int32Builder::new(capacity),
            relchecks: Int32Builder::new(capacity),
            relhasrules: BooleanBuilder::new(capacity),
            relhastriggers: BooleanBuilder::new(capacity),
            relhassubclass: BooleanBuilder::new(capacity),
            relrowsecurity: BooleanBuilder::new(capacity),
            relforcerowsecurity: BooleanBuilder::new(capacity),
            relispopulated: BooleanBuilder::new(capacity),
            relreplident: StringBuilder::new(capacity),
            relispartition: BooleanBuilder::new(capacity),
            relrewrite: UInt32Builder::new(capacity),
            relfrozenxid: Int32Builder::new(capacity),
            relminmxid: Int32Builder::new(capacity),
            relacl: StringBuilder::new(capacity),
            reloptions: StringBuilder::new(capacity),
            relpartbound: StringBuilder::new(capacity),
            xmin: UInt32Builder::new(capacity),
            relhasoids: BooleanBuilder::new(capacity),
        }
    }

    fn add_class(&mut self, class: &PgClass) {
        self.oid.append_value(class.oid).unwrap();
        self.relname.append_value(&class.relname).unwrap();
        self.relnamespace.append_value(class.relnamespace).unwrap();
        self.reltype.append_value(class.reltype).unwrap();
        self.reloftype.append_value(0).unwrap();
        self.relowner.append_value(10).unwrap();
        self.relam.append_value(class.relam).unwrap();
        self.relfilenode.append_value(class.relfilenode).unwrap();
        self.reltablespace.append_value(0).unwrap();
        self.relpages.append_value(0).unwrap();
        self.reltuples.append_value(-1).unwrap();
        self.relallvisible.append_value(0).unwrap();
        self.reltoastrelid
            .append_value(class.reltoastrelid)
            .unwrap();
        self.relhasindex.append_value(false).unwrap();
        self.relisshared.append_value(class.relisshared).unwrap();
        self.relpersistence.append_value("p").unwrap();
        self.relkind.append_value(&class.relkind).unwrap();
        self.relnatts.append_value(class.relnatts).unwrap();
        self.relchecks.append_value(0).unwrap();
        self.relhasrules.append_value(class.relhasrules).unwrap();
        self.relhastriggers.append_value(false).unwrap();
        self.relhassubclass.append_value(false).unwrap();
        self.relrowsecurity.append_value(false).unwrap();
        self.relforcerowsecurity.append_value(false).unwrap();
        self.relispopulated.append_value(true).unwrap();
        self.relreplident.append_value(&class.relreplident).unwrap();
        self.relispartition.append_value(false).unwrap();
        self.relrewrite.append_value(0).unwrap();
        self.relfrozenxid.append_value(class.relfrozenxid).unwrap();
        self.relminmxid.append_value(class.relminmxid).unwrap();
        self.relacl.append_null().unwrap();
        self.reloptions.append_null().unwrap();
        self.relpartbound.append_null().unwrap();
        self.xmin.append_value(1).unwrap();
        self.relhasoids.append_value(false).unwrap();
    }

    fn finish(mut self) -> Vec<Arc<dyn Array>> {
        let columns: Vec<Arc<dyn Array>> = vec![
            Arc::new(self.oid.finish()),
            Arc::new(self.relname.finish()),
            Arc::new(self.relnamespace.finish()),
            Arc::new(self.reltype.finish()),
            Arc::new(self.reloftype.finish()),
            Arc::new(self.relowner.finish()),
            Arc::new(self.relam.finish()),
            Arc::new(self.relfilenode.finish()),
            Arc::new(self.reltablespace.finish()),
            Arc::new(self.relpages.finish()),
            Arc::new(self.reltuples.finish()),
            Arc::new(self.relallvisible.finish()),
            Arc::new(self.reltoastrelid.finish()),
            Arc::new(self.relhasindex.finish()),
            Arc::new(self.relisshared.finish()),
            Arc::new(self.relpersistence.finish()),
            Arc::new(self.relkind.finish()),
            Arc::new(self.relnatts.finish()),
            Arc::new(self.relchecks.finish()),
            Arc::new(self.relhasrules.finish()),
            Arc::new(self.relhastriggers.finish()),
            Arc::new(self.relhassubclass.finish()),
            Arc::new(self.relrowsecurity.finish()),
            Arc::new(self.relforcerowsecurity.finish()),
            Arc::new(self.relispopulated.finish()),
            Arc::new(self.relreplident.finish()),
            Arc::new(self.relispartition.finish()),
            Arc::new(self.relrewrite.finish()),
            Arc::new(self.relfrozenxid.finish()),
            Arc::new(self.relminmxid.finish()),
            Arc::new(self.relacl.finish()),
            Arc::new(self.reloptions.finish()),
            Arc::new(self.relpartbound.finish()),
            Arc::new(self.xmin.finish()),
            Arc::new(self.relhasoids.finish()),
        ];

        columns
    }
}

pub struct PgCatalogClassProvider {
    data: Arc<Vec<ArrayRef>>,
}

impl PgCatalogClassProvider {
    pub fn new(cube_tables: &[CubeMetaTable]) -> Self {
        let mut builder = PgCatalogClassBuilder::new();

        // TODO add all pg_catalog tables to pg_class

        // See https://github.com/postgres/postgres/blob/REL_16_4/src/include/catalog/pg_class.h#L32-L142
        // See https://github.com/postgres/postgres/blob/REL_16_4/src/include/catalog/pg_class.dat
        builder.add_class(&PgClass {
            oid: PG_CLASS_CLASS_OID,
            relname: "pg_class".to_string(),
            relnamespace: PG_NAMESPACE_CATALOG_OID,
            reltype: PG_CLASS_ROWTYPE_OID,
            relam: 2,
            relfilenode: 0,
            reltoastrelid: 0,
            relisshared: false,
            relkind: "r".to_string(),
            // Number of fields in PgCatalogClassProvider::schema()
            relnatts: 34,
            relhasrules: false,
            relreplident: "p".to_string(),
            relfrozenxid: 0,
            relminmxid: 1,
        });

        for table in cube_tables.iter() {
            builder.add_class(&PgClass {
                oid: table.oid,
                relname: table.name.clone(),
                relnamespace: PG_NAMESPACE_PUBLIC_OID,
                reltype: table.record_oid,
                relam: 2,
                relfilenode: 0,
                reltoastrelid: 0,
                relisshared: false,
                relkind: "r".to_string(),
                relnatts: table.columns.len().to_i32().unwrap_or(0),
                relhasrules: false,
                relreplident: "p".to_string(),
                relfrozenxid: 0,
                relminmxid: 1,
            });
        }

        Self {
            data: Arc::new(builder.finish()),
        }
    }
}

#[async_trait]
impl TableProvider for PgCatalogClassProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn table_type(&self) -> TableType {
        TableType::View
    }

    fn schema(&self) -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("oid", DataType::UInt32, false),
            Field::new("relname", DataType::Utf8, false),
            // info_schma: 13000; pg_catalog: 11; user defined tables: 2200
            Field::new("relnamespace", DataType::UInt32, false),
            Field::new("reltype", DataType::UInt32, false),
            Field::new("reloftype", DataType::UInt32, false),
            Field::new("relowner", DataType::UInt32, false),
            //user defined tables: 2; system tables: 0 | 2
            Field::new("relam", DataType::UInt32, false),
            // TODO: to check that 0 if fine
            Field::new("relfilenode", DataType::UInt32, false),
            Field::new("reltablespace", DataType::UInt32, false),
            Field::new("relpages", DataType::Int32, false),
            Field::new("reltuples", DataType::Int32, false),
            Field::new("relallvisible", DataType::Int32, false),
            // TODO: sometimes is not 0. Check that 0 is fine
            Field::new("reltoastrelid", DataType::UInt32, false),
            Field::new("relhasindex", DataType::Boolean, false),
            //user defined tables: FALSE; system tables: FALSE | TRUE
            Field::new("relisshared", DataType::Boolean, false),
            Field::new("relpersistence", DataType::Utf8, false),
            // Tables: r; Views: v
            Field::new("relkind", DataType::Utf8, false),
            // number of columns in table
            Field::new("relnatts", DataType::Int32, false),
            Field::new("relchecks", DataType::Int32, false),
            //user defined tables: FALSE; system tables: FALSE | TRUE
            Field::new("relhasrules", DataType::Boolean, false),
            Field::new("relhastriggers", DataType::Boolean, false),
            Field::new("relhassubclass", DataType::Boolean, false),
            Field::new("relrowsecurity", DataType::Boolean, false),
            Field::new("relforcerowsecurity", DataType::Boolean, false),
            Field::new("relispopulated", DataType::Boolean, false),
            //user defined tables: p; system tables: n
            Field::new("relreplident", DataType::Utf8, false),
            Field::new("relispartition", DataType::Boolean, false),
            Field::new("relrewrite", DataType::UInt32, false),
            // TODO: can be not 0; check that 0 is fine
            Field::new("relfrozenxid", DataType::Int32, false),
            // Tables: 1; Other: v
            Field::new("relminmxid", DataType::Int32, false),
            Field::new("relacl", DataType::Utf8, true),
            Field::new("reloptions", DataType::Utf8, true),
            Field::new("relpartbound", DataType::Utf8, true),
            Field::new("xmin", DataType::UInt32, false),
            Field::new("relhasoids", DataType::Boolean, false),
        ]))
    }

    async fn scan(
        &self,
        projection: &Option<Vec<usize>>,
        _filters: &[Expr],
        _limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
        let batch = RecordBatch::try_new(self.schema(), self.data.to_vec())?;

        Ok(Arc::new(MemoryExec::try_new(
            &[vec![batch]],
            self.schema(),
            projection.clone(),
        )?))
    }

    fn supports_filter_pushdown(
        &self,
        _filter: &Expr,
    ) -> Result<TableProviderFilterPushDown, DataFusionError> {
        Ok(TableProviderFilterPushDown::Unsupported)
    }
}
