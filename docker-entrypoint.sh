#!/busybox/sh
set -e

DB_URL="${DATABASE_URL:-sqlite:/app/data/flashy.db}"
DB_PATH=$(echo "$DB_URL" | sed 's|^sqlite:||')
DB_DIR=$(dirname "$DB_PATH")

if ! mkdir -p "$DB_DIR" 2>/dev/null; then
    echo "ERROR: Cannot create data directory: $DB_DIR"
    echo "Please ensure the volume is mounted with proper permissions"
    exit 1
fi

if [ -f "$DB_PATH" ]; then
    echo "Database file found - will run migrations to ensure schema is up-to-date"
    DB_EXISTS=true
else
    echo "Database file not found - will create new database and run initial migrations"
    DB_EXISTS=false
fi

echo "========================================="
echo "Running database setup..."
echo "========================================="

cd /app

if [ "$DB_EXISTS" = false ]; then
    echo "Creating new database..."
    /app/sqlx database create
    echo "Database created successfully"
    
    echo "Running initial migrations..."
    /app/sqlx migrate run
    echo "Initial migrations completed"
else
    echo "Database already exists, checking for new migrations..."
    /app/sqlx migrate run
    echo "Migrations up to date"
fi

echo "========================================="
echo "Starting Flashy application..."
echo "========================================="

# Execute the application (replaces this shell process)
exec /app/flashy
