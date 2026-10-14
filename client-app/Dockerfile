# syntax=docker/dockerfile:1.7
# The SvelteKit app, kept running behind the Rust server until its last page
# is migrated (slice S20 deletes this file). Build context: client-app/.

FROM node:20.19-bookworm-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
# $env/static/private is inlined at build time, so the API address is baked
# into the bundle: the Rust service as seen from inside the compose network.
ARG PRIVATE_API_URL=http://web:8080
ENV PRIVATE_API_URL=$PRIVATE_API_URL
RUN npm run build && npm prune --omit=dev

FROM node:20.19-bookworm-slim
WORKDIR /app
ENV NODE_ENV=production \
    PORT=3001
COPY --from=build /app/package.json ./
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/build ./build
USER node
EXPOSE 3001
CMD ["node", "build"]
