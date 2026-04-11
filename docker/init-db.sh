#!/bin/bash
# Creates the development database alongside the default one.
# Mounted into Postgres via docker-compose initdb volume.
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    SELECT 'CREATE DATABASE orbflow_dev OWNER orbflow'
    WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'orbflow_dev')\gexec
EOSQL
