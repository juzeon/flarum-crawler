# flarum-crawler

An integrated crawler for [Flarum](https://flarum.org/) forum.

## Features

- Crawling with customized concurrency
- Data deduplication
- Saving data to a SQLite database

## Build

```bash
cargo build -r
```

## Configuration

`config.yml`:

```yaml
base_url: https://forum.example.com
concurrency: 3
db: data.db
```

## Database

```sql
-- ----------------------------
-- Table structure for discussions
-- ----------------------------
DROP TABLE IF EXISTS "discussions";
CREATE TABLE "discussions" (
  "id" INTEGER NOT NULL,
  "user_id" INTEGER NOT NULL,
  "username" TEXT NOT NULL,
  "user_display_name" TEXT NOT NULL,
  "title" TEXT NOT NULL,
  "tags" TEXT NOT NULL,
  "is_frontpage" integer NOT NULL,
  "created_at" TEXT NOT NULL,
  PRIMARY KEY ("id")
);

-- ----------------------------
-- Table structure for jobs
-- ----------------------------
DROP TABLE IF EXISTS "jobs";
CREATE TABLE "jobs" (
  "entity" TEXT NOT NULL,
  "entity_id" INTEGER NOT NULL,
  "status" TEXT NOT NULL,
  PRIMARY KEY ("entity", "entity_id")
);

-- ----------------------------
-- Table structure for posts
-- ----------------------------
DROP TABLE IF EXISTS "posts";
CREATE TABLE "posts" (
  "id" INTEGER NOT NULL,
  "user_id" INTEGER NOT NULL,
  "discussion_id" INTEGER NOT NULL,
  "reply_to_id" INTEGER NOT NULL,
  "username" TEXT NOT NULL,
  "user_display_name" TEXT NOT NULL,
  "content" TEXT NOT NULL,
  "created_at" TEXT NOT NULL,
  PRIMARY KEY ("id")
);
```

## Usage

Execute `flarum-crawler -h` for detailed help information.

