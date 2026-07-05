import type { HydratedDocument, Types } from 'mongoose';
import { Schema, model } from 'mongoose';

export interface FolderDocument {
  name: string;
  parentId: Types.ObjectId | null;
  ownerId: string;
  isPublic: boolean;
  createdAt: Date;
  updatedAt: Date;
}

export type FolderHydrated = HydratedDocument<FolderDocument>;

const folderSchema = new Schema<FolderDocument>(
  {
    name: { type: String, required: true, trim: true },
    parentId: { type: Schema.Types.ObjectId, ref: 'Folder', default: null, index: true },
    ownerId: { type: String, required: true, index: true },
    isPublic: { type: Boolean, default: false, index: true },
  },
  { timestamps: true },
);

folderSchema.index({ ownerId: 1, parentId: 1 });

export const FolderModel = model<FolderDocument>('Folder', folderSchema);
