import { PutBucketCorsCommand } from '@aws-sdk/client-s3';
import { env } from '../src/config/env';
import { r2Client } from '../src/config/r2';

async function setupCors() {
  try {
    const command = new PutBucketCorsCommand({
      Bucket: env.R2_BUCKET_NAME,
      CORSConfiguration: {
        CORSRules: [
          {
            AllowedHeaders: ['*'],
            AllowedMethods: ['GET', 'PUT', 'POST', 'DELETE', 'HEAD'],
            AllowedOrigins: ['*'], // You can restrict this to 'http://localhost:4000' in production
            ExposeHeaders: ['ETag'],
            MaxAgeSeconds: 3000,
          },
        ],
      },
    });

    await r2Client.send(command);
    console.log('CORS rules successfully applied to the bucket:', env.R2_BUCKET_NAME);
  } catch (err) {
    console.error('Error applying CORS rules:', err);
    process.exit(1);
  }
}

setupCors();
