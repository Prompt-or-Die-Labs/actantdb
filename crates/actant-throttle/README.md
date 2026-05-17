# actant-throttle

Multi-axis rate limiting + autonomy throttling. Per actor / agent / subagent / tenant / workflow / tool / provider / model / API key / worker / resource / sensitivity / cost-center. Algorithms: token bucket, leaky bucket, fixed window, sliding window, concurrency semaphore, weighted fair queue, priority queue, deadline queue, adaptive provider-rate tracking. Ingests `RateLimit-*` HTTP headers from upstream providers.

See `agents/actant-throttle.md`.
