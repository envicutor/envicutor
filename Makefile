start:
	docker compose up -d --build
	make logs

logs:
	docker compose logs -f

stop:
	docker compose down
