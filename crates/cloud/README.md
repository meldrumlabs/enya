# Enya Cloud

Collaboration backend for Enya Editor teams.

## License

**Proprietary Software** - Copyright (c) 2026 Meldrum Labs AB

This software requires a commercial license for use. See [LICENSE](LICENSE) for details.

For licensing inquiries: contact@meldrumlabs.com

## Features

- GitHub OAuth authentication
- Team management with role-based access (Admin/Member)
- Real-time collaboration via WebSocket
- Threaded discussions and annotations
- Audit logging for compliance
- Prometheus metrics

## Documentation

- [DEPLOYMENT.md](DEPLOYMENT.md) - Self-hosted deployment guide
- [METRICS.md](METRICS.md) - Prometheus metrics reference
- [DESIGN.md](DESIGN.md) - Architecture overview

## Quick Start

```bash
# Configure environment
cp .env.example .env
# Edit .env with your settings (JWT_SECRET, etc.)

# Run with Docker Compose
docker-compose up -d

# Verify
curl http://localhost:3000/health
```

## Development

```bash
# Run locally (requires PostgreSQL)
DATABASE_URL="postgres://..." cargo run -p enya-cloud

# Run tests
cargo nextest run -p enya-cloud
```
