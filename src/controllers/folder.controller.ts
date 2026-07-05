import type { Request, Response } from 'express';
import { z } from 'zod';

import * as folderService from '../services/folder.service';

const listQuerySchema = z.object({
  parentId: z.string().optional(),
});

const createBodySchema = z.object({
  name: z.string().trim().min(1, 'Tên thư mục không được để trống'),
  parentId: z.string().nullable().optional(),
  isPublic: z.boolean().optional().default(false),
});

const updateBodySchema = z.object({
  name: z.string().trim().min(1).optional(),
  parentId: z.string().nullable().optional(),
  isPublic: z.boolean().optional(),
});

export const listFolders = async (req: Request, res: Response): Promise<void> => {
  const { parentId } = listQuerySchema.parse(req.query);
  const folders = await folderService.listFolders({
    parentId: parentId ?? null,
    viewerId: req.user?.id,
  });
  res.json(folders);
};

export const createFolder = async (req: Request, res: Response): Promise<void> => {
  const body = createBodySchema.parse(req.body);
  const folder = await folderService.createFolder({
    name: body.name,
    parentId: body.parentId ?? null,
    isPublic: body.isPublic,
    ownerId: req.user!.id,
  });
  res.status(201).json(folder);
};

export const updateFolder = async (req: Request, res: Response): Promise<void> => {
  const body = updateBodySchema.parse(req.body);
  const folder = await folderService.updateFolder(req.params.id, req.user!.id, body);
  res.json(folder);
};

export const deleteFolder = async (req: Request, res: Response): Promise<void> => {
  await folderService.deleteFolderRecursive(req.params.id, req.user!.id);
  res.status(204).send();
};
