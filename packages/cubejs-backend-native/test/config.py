from cube.conf import (
    settings,
    file_repository
)

settings.schema_path = "models"
settings.pg_sql_port = 5555
settings.telemetry = False

def query_rewrite(query, ctx):
    print('[python] query_rewrite query=', query, ' ctx=', ctx)
    return query

settings.query_rewrite = query_rewrite

async def check_auth(req, authorization):
    print('[python] check_auth req=', req, ' authorization=', authorization)


settings.check_auth = check_auth

async def repository_factory(ctx):
    print('[python] repository_factory ctx=', ctx)

    return file_repository(ctx['securityContext']['schemaPath'])

settings.repository_factory = repository_factory

async def context_to_api_scopes():
    print('[python] context_to_api_scopes')
    return ['meta', 'data', 'jobs']

settings.context_to_api_scopes = context_to_api_scopes
