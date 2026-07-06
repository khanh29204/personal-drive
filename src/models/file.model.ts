import type { HydratedDocument, Types } from 'mongoose';
import { Schema, model } from 'mongoose';

export type FileStatus = 'pending' | 'completed' | 'failed';

export interface FileDocument {
  name: string;
  key: string;
  size: number;
  mimeType: string;
  externalUrl?: string | null;
  folderId: Types.ObjectId | null;
  ownerId: string;
  isPublic: boolean;
  status: FileStatus;
  views: number;
  downloads: number;
  createdAt: Date;
  updatedAt: Date;
}

export type FileHydrated = HydratedDocument<FileDocument>;

const fileSchema = new Schema<FileDocument>(
  {
    name: { type: String, required: true, trim: true },
    key: { type: String, required: true, unique: true },
    size: { type: Number, required: true, min: 0 },
    mimeType: { type: String, required: true },
    externalUrl: { type: String, default: null },
    folderId: { type: Schema.Types.ObjectId, ref: 'Folder', default: null, index: true },
    ownerId: { type: String, required: true, index: true },
    isPublic: { type: Boolean, default: false, index: true },
    status: {
      type: String,
      enum: ['pending', 'completed', 'failed'],
      default: 'pending',
      index: true,
    },
    views: { type: Number, default: 0 },
    downloads: { type: Number, default: 0 },
  },
  { timestamps: true },
);

fileSchema.index({ ownerId: 1, folderId: 1, status: 1 });

export const FileModel = model<FileDocument>('File', fileSchema);
