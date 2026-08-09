import * as fs from 'fs';
import * as path from 'path';

import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';

import { env } from '../config/env';

let PROTO_PATH = path.join(__dirname, '../../proto/auth.proto');
if (!fs.existsSync(PROTO_PATH)) {
  PROTO_PATH = path.join(__dirname, '../../../proto/auth.proto');
}

const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});

const protoDescriptor = grpc.loadPackageDefinition(packageDefinition);
const authProto: any = protoDescriptor.auth;

export const authGrpcClient = new authProto.AuthService(
  env.AUTH_GRPC_BASE_URL,
  grpc.credentials.createInsecure()
);

export const verifyTokenGrpc = (token: string): Promise<{ valid: boolean; user_id: string; user_name: string }> => {
  return new Promise((resolve, reject) => {
    authGrpcClient.VerifyToken({ token }, (error: any, response: any) => {
      if (error) {
        return reject(error);
      }
      resolve(response);
    });
  });
};
