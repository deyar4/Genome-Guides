# Web image: Next.js. NEXT_PUBLIC_API_URL is compiled into the client bundle, so it is a build argument: it must be the
# URL the BROWSER uses to reach the API (e.g. https://guides.example.org/api/v1 behind a reverse proxy).
#   docker build -f deploy/web.Dockerfile --build-arg NEXT_PUBLIC_API_URL=http://localhost:8000/api/v1 -t genome-guides-web .
FROM node:24-slim
RUN npm install -g pnpm@12.4.1
WORKDIR /srv
COPY pnpm-workspace.yaml pnpm-lock.yaml ./
COPY apps/web/package.json apps/web/
RUN pnpm install --frozen-lockfile --filter web...
COPY apps/web apps/web
ARG NEXT_PUBLIC_API_URL=http://localhost:8000/api/v1
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL \
    NEXT_TELEMETRY_DISABLED=1
RUN pnpm --filter web run build && chown -R node:node apps/web/.next
WORKDIR /srv/apps/web
USER node
EXPOSE 3000
# next is started directly: pnpm exec would try to write into the root-owned node_modules
CMD ["node_modules/.bin/next", "start", "-p", "3000"]
