.PHONY: dev services services-down build db-init db-drop db-seed test healthcheck

dev:
	PRETTY_LOGS=true cargo run

services:
	docker compose up -d

services-down:
	docker compose down

build:
	cargo build --release

db-init:
	surreal import --endpoint http://localhost:8001 \
		--username root --password root \
		--namespace pavilion --database pavilion \
		db/schema.surql

db-drop:
	echo "USE NS pavilion; REMOVE DATABASE pavilion;" | surreal sql \
		--endpoint http://localhost:8001 \
		--username root --password root

db-seed:
	surreal import --endpoint http://localhost:8001 \
		--username root --password root \
		--namespace pavilion --database pavilion \
		db/seed.surql

test:
	cargo test --workspace

healthcheck:
	curl -s http://localhost:3000/healthcheck | jq .
