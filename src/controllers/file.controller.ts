import type { Request, Response } from 'express';
import { z } from 'zod';

import * as fileService from '../services/file.service';

const listQuerySchema = z.object({
  folderId: z.string().optional(),
});

const uploadUrlBodySchema = z.object({
  name: z.string().trim().min(1, 'Tên file không được để trống'),
  mimeType: z.string().trim().min(1, 'mimeType không được để trống'),
  size: z.number().int().nonnegative(),
  folderId: z.string().nullable().optional(),
  isPublic: z.boolean().optional().default(false),
});

const updateBodySchema = z.object({
  name: z.string().trim().min(1).optional(),
  isPublic: z.boolean().optional(),
  folderId: z.string().nullable().optional(),
});

export const listFiles = async (req: Request, res: Response): Promise<void> => {
  const { folderId } = listQuerySchema.parse(req.query);
  const files = await fileService.listFiles({
    folderId: folderId ?? null,
    viewerId: req.user?.id,
  });
  res.json(files);
};

export const requestUploadUrl = async (req: Request, res: Response): Promise<void> => {
  const body = uploadUrlBodySchema.parse(req.body);
  const result = await fileService.requestUploadUrl({
    name: body.name,
    mimeType: body.mimeType,
    size: body.size,
    folderId: body.folderId ?? null,
    isPublic: body.isPublic,
    ownerId: req.user!.id,
  });
  res.status(201).json(result);
};

export const completeUpload = async (req: Request, res: Response): Promise<void> => {
  const file = await fileService.completeUpload(req.params.id, req.user!.id);
  res.json(file);
};

export const getDownloadUrl = async (req: Request, res: Response): Promise<void> => {
  const url = await fileService.getDownloadUrl(req.params.id, req.user?.id);
  res.json({ downloadUrl: url });
};

export const deleteFile = async (req: Request, res: Response): Promise<void> => {
  await fileService.deleteFile(req.params.id, req.user!.id);
  res.status(204).send();
};

export const updateFile = async (req: Request, res: Response): Promise<void> => {
  const body = updateBodySchema.parse(req.body);
  const file = await fileService.updateFile(req.params.id, req.user!.id, body);
  res.json(file);
};
