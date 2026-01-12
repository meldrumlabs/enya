# Enya Cloud - Self-Hosted Deployment Guide

This guide covers deploying Enya Cloud on your own infrastructure.

## Quick Start (Docker Compose)

### Prerequisites

- Docker 20.10+
- Docker Compose 2.0+

### Steps

1. **Clone and navigate to the cloud directory**
   ```bash
   cd crates/cloud
   ```

2. **Configure environment**
   ```bash
   cp .env.example .env

   # Generate a secure JWT secret
   openssl rand -hex 32
   # Copy the output to JWT_SECRET in .env
   ```

3. **Edit `.env`** with your configuration:
   ```bash
   # Required
   JWT_SECRET=<your-generated-secret>

   # Recommended for production
   POSTGRES_PASSWORD=<strong-password>
   FRONTEND_URL=https://your-frontend-domain.com

   # Optional: GitHub OAuth
   GITHUB_CLIENT_ID=<your-client-id>
   GITHUB_CLIENT_SECRET=<your-client-secret>
   ```

4. **Start the services**
   ```bash
   docker-compose up -d
   ```

5. **Verify deployment**
   ```bash
   curl http://localhost:3000/health
   # Should return: ok
   ```

## Production Deployment

### Security Checklist

- [ ] Generate a strong JWT_SECRET (32+ bytes)
- [ ] Use a strong POSTGRES_PASSWORD
- [ ] Set DEV_AUTH=false (disable dev login endpoint)
- [ ] Configure HTTPS via reverse proxy (nginx, Caddy, Traefik)
- [ ] Restrict database port access (don't expose 5432 publicly)
- [ ] Set up database backups
- [ ] Configure log aggregation

### Reverse Proxy (HTTPS)

Example nginx configuration:

```nginx
server {
    listen 443 ssl http2;
    server_name api.your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Database Backups

Automated daily backup script:

```bash
#!/bin/bash
BACKUP_DIR=/backups/enya
DATE=$(date +%Y%m%d_%H%M%S)

docker exec enya-postgres pg_dump -U enya enya | gzip > $BACKUP_DIR/enya_$DATE.sql.gz

# Keep last 30 days
find $BACKUP_DIR -name "*.sql.gz" -mtime +30 -delete
```

### Resource Requirements

| Component | CPU | Memory | Storage |
|-----------|-----|--------|---------|
| API Server | 0.5 vCPU | 256MB | - |
| PostgreSQL | 0.5 vCPU | 512MB | 10GB+ |

Scale based on team size:
- Small team (<20 users): 1 vCPU, 1GB RAM total
- Medium team (20-100 users): 2 vCPU, 2GB RAM total
- Large team (100+ users): 4 vCPU, 4GB RAM total

## External PostgreSQL

To use an external PostgreSQL instance (e.g., AWS RDS, PlanetScale):

1. Remove the `postgres` service from docker-compose.yml

2. Update `.env`:
   ```bash
   DATABASE_URL=postgres://user:password@your-db-host:5432/enya
   ```

3. Run migrations manually:
   ```bash
   # Install sqlx-cli
   cargo install sqlx-cli

   # Run migrations
   DATABASE_URL="your-connection-string" sqlx migrate run
   ```

## Kubernetes Deployment

For Kubernetes deployments, we recommend:

1. Use a Helm chart (coming soon)
2. Or create your own manifests based on docker-compose.yml
3. Use a managed PostgreSQL service
4. Configure Ingress for HTTPS

Example deployment.yaml:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: enya-cloud
spec:
  replicas: 2
  selector:
    matchLabels:
      app: enya-cloud
  template:
    metadata:
      labels:
        app: enya-cloud
    spec:
      containers:
      - name: enya-cloud
        image: enya-cloud:latest
        ports:
        - containerPort: 3000
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: enya-secrets
              key: database-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: enya-secrets
              key: jwt-secret
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 10
```

## Troubleshooting

### API server won't start

1. Check database connectivity:
   ```bash
   docker-compose logs postgres
   docker-compose exec postgres pg_isready
   ```

2. Verify migrations ran:
   ```bash
   docker-compose logs cloud | grep -i migration
   ```

### Database connection errors

1. Ensure postgres is healthy:
   ```bash
   docker-compose ps
   ```

2. Check DATABASE_URL format:
   ```
   postgres://USER:PASSWORD@HOST:PORT/DATABASE
   ```

### OAuth not working

1. Verify callback URL matches GitHub app settings
2. Check GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET are set
3. Ensure FRONTEND_URL is correct for redirects

## Monitoring

### Health Check Endpoint

```bash
GET /health
# Returns: ok
```

### Logs

```bash
# View all logs
docker-compose logs -f

# View API logs only
docker-compose logs -f cloud

# View database logs only
docker-compose logs -f postgres
```

### Prometheus Metrics

Enya Cloud exposes Prometheus metrics at `/metrics`. See [METRICS.md](METRICS.md) for a complete list of available metrics.

#### Quick Setup with Prometheus

Add Enya Cloud to your Prometheus scrape config:

```yaml
scrape_configs:
  - job_name: 'enya-cloud'
    static_configs:
      - targets: ['enya-cloud:3000']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

#### Docker Compose with Monitoring Stack

For a complete monitoring setup, add Prometheus and Grafana to your docker-compose.yml:

```yaml
services:
  # ... existing services ...

  prometheus:
    image: prom/prometheus:v2.48.0
    container_name: enya-prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=30d'
    restart: unless-stopped

  grafana:
    image: grafana/grafana:10.2.0
    container_name: enya-grafana
    ports:
      - "3001:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: ${GRAFANA_PASSWORD:-admin}
    volumes:
      - grafana_data:/var/lib/grafana
    depends_on:
      - prometheus
    restart: unless-stopped

volumes:
  prometheus_data:
  grafana_data:
```

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'enya-cloud'
    static_configs:
      - targets: ['cloud:3000']
    metrics_path: '/metrics'
```

#### Key Metrics to Monitor

| Metric | Purpose | Alert Threshold |
|--------|---------|-----------------|
| `http_requests_total{status=~"5.."}` | Server errors | > 1% of requests |
| `http_request_duration_seconds` | Latency | p95 > 1s |
| `enya_db_pool_connections_idle` | DB pool health | = 0 for > 1m |
| `enya_websocket_connections` | Active realtime users | Monitor trends |

#### Example Alerts

Add to your Prometheus alerting rules:

```yaml
groups:
  - name: enya-cloud
    rules:
      - alert: HighErrorRate
        expr: sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m])) > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate (>1%)"

      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High p95 latency (>1s)"

      - alert: DatabasePoolExhausted
        expr: enya_db_pool_connections_idle == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "No idle database connections"
```

## Enterprise Features

Self-hosted Enya Cloud includes these enterprise features out of the box:

### Audit Logging

All team actions are logged with actor, timestamp, and details:
- Member invitations and role changes
- Team settings modifications
- Channel and thread creation
- Admin actions

Access audit logs via the API (admin-only):

```bash
GET /teams/{team_id}/audit-logs?limit=50&offset=0
```

Returns:
```json
{
  "logs": [
    {
      "id": "uuid",
      "actor_id": "user-uuid",
      "actor_name": "Alice",
      "action": "member_invited",
      "resource_type": "invitation",
      "resource_id": "invitation-uuid",
      "details": {"email": "bob@company.com", "role": "member"},
      "created_at": 1704067200
    }
  ],
  "total": 150,
  "limit": 50,
  "offset": 0
}
```

### Role-Based Access Control

- **Admin**: Full team management (invitations, role changes, audit logs)
- **Member**: Standard collaboration features

### Data Ownership

With self-hosted deployment:
- All data stays on your infrastructure
- Full database access for backups and compliance
- No data sent to external services
- GDPR/HIPAA-ready deployment

### WebSocket Real-Time

Real-time collaboration via WebSocket:
- Live message updates
- Typing indicators
- Thread resolution notifications
- Annotation changes

Connect at: `wss://your-domain.com/ws?token={jwt}&team_id={uuid}`

## Support

For enterprise support, contact: [your-email]
