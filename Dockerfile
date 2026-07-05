# Stage 1: Build
FROM node:20-alpine AS builder

# Set working directory
WORKDIR /app

# Copy package management files
COPY package.json package-lock.json* yarn.lock* ./

# Install dependencies
RUN npm ci || npm install

# Copy source code
COPY . .

# Build the project (TypeScript to JS + copy assets)
RUN npm run build

# Stage 2: Production
FROM node:20-alpine AS runner

# Set working directory
WORKDIR /app

# Set node environment to production
ENV NODE_ENV=production

# Copy package management files
COPY package.json package-lock.json* yarn.lock* ./

# Install only production dependencies
RUN npm ci --only=production || npm install --omit=dev

# Copy the built dist folder from the builder stage
COPY --from=builder /app/dist ./dist

# Create a non-root user and group for security
RUN addgroup -S nodejs && adduser -S nodejs -G nodejs
USER nodejs

# Expose the default port
EXPOSE 4000

# Start the application
CMD ["npm", "start"]
