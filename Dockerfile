# ============================================================
# Stage 1: Install dependencies
# ============================================================
FROM node:20-alpine AS deps

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/package.json
COPY packages/orbflow-core/package.json packages/orbflow-core/package.json

RUN pnpm install --frozen-lockfile

# ============================================================
# Stage 2: Build
# ============================================================
FROM node:20-alpine AS builder

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/apps/web/node_modules ./apps/web/node_modules
COPY --from=deps /app/packages/orbflow-core/node_modules ./packages/orbflow-core/node_modules
COPY . .

ENV NEXT_TELEMETRY_DISABLED=1
RUN pnpm build

# ============================================================
# Stage 3: Production runtime
# ============================================================
FROM node:20-alpine AS runner

RUN addgroup -g 1001 -S orbflow && \
    adduser -S -u 1001 -G orbflow orbflow

WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

COPY --from=builder --chown=orbflow:orbflow /app/apps/web/.next/standalone ./
COPY --from=builder --chown=orbflow:orbflow /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder --chown=orbflow:orbflow /app/apps/web/public ./apps/web/public

USER orbflow
EXPOSE 3000

ENV HOSTNAME="0.0.0.0"
ENV PORT=3000

CMD ["node", "apps/web/server.js"]
