import type { Request, Response } from 'express';

import * as fileService from '../services/file.service';
import * as folderService from '../services/folder.service';
import { formatBytes, getFileIcon } from '../utils/fileDisplay';

export interface DisplayItem {
  id: string;
  name: string;
  isDirectory: boolean;
  icon: string;
  sizeLabel: string;
  sizeRaw: number;
  mimeType: string;
  modified: Date;
  href: string;
  downloadHref: string | null;
  isPublic: boolean;
  isOwner: boolean;
  externalUrl?: string | null;
}

export const renderHome = async (req: Request, res: Response): Promise<void> => {
  let folderId = typeof req.query.folderId === 'string' ? req.query.folderId : null;
  const dirPath = typeof req.query.dir === 'string' ? req.query.dir : null;
  const viewerId = req.user?.id;

  if (!folderId && dirPath) {
    const resolvedId = await folderService.resolvePath(dirPath, viewerId);
    if (resolvedId) {
      folderId = resolvedId;
    }
  }

  const sortBy = (
    ['name', 'type', 'size', 'date'].includes(req.query.sortBy as string)
      ? (req.query.sortBy as string)
      : 'name'
  ) as 'name' | 'type' | 'size' | 'date';
  const order = req.query.order === 'desc' ? 'desc' : 'asc';

  const breadcrumb = await folderService.getBreadcrumb(folderId, viewerId);
  const [folders, files] = await Promise.all([
    folderService.listFolders({ parentId: folderId, viewerId }),
    fileService.listFiles({ folderId, viewerId }),
  ]);

  const currentPath = breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].path : '';

  const folderItems: DisplayItem[] = folders.map((folder) => {
    const itemPath = currentPath ? `${currentPath}/${folder.name}` : folder.name;
    return {
      id: String(folder._id),
      name: folder.name,
      isDirectory: true,
      icon: 'fa-folder',
      sizeLabel: '-',
      sizeRaw: -1,
      mimeType: '',
      modified: folder.updatedAt,
      href: `/?dir=${encodeURIComponent(itemPath)}`,
      downloadHref: null,
      isPublic: folder.isPublic,
      isOwner: folder.ownerId === viewerId,
    };
  });

  const fileItems: DisplayItem[] = files.map((file) => ({
    id: String(file._id),
    name: file.name,
    isDirectory: false,
    icon: getFileIcon(file.mimeType),
    sizeLabel: formatBytes(file.size),
    sizeRaw: file.size,
    mimeType: file.mimeType,
    modified: file.updatedAt,
    href: `/files/${file._id}/view`,
    downloadHref: `/files/${file._id}/download`,
    isPublic: file.isPublic,
    isOwner: file.ownerId === viewerId,
    externalUrl: file.externalUrl,
  }));

  // ── sort ──────────────────────────────────────────────────────────
  const comparator = (a: DisplayItem, b: DisplayItem): number => {
    let va: string | number;
    let vb: string | number;
    switch (sortBy) {
      case 'type':
        va = a.isDirectory ? a.name.toLowerCase() : a.mimeType;
        vb = b.isDirectory ? b.name.toLowerCase() : b.mimeType;
        break;
      case 'size':
        if (a.isDirectory) return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
        va = a.sizeRaw;
        vb = b.sizeRaw;
        break;
      case 'date':
        va = a.modified.getTime();
        vb = b.modified.getTime();
        break;
      default:
        va = a.name.toLowerCase();
        vb = b.name.toLowerCase();
    }
    const cmp = typeof va === 'number' ? va - (vb as number) : va.localeCompare(vb as string);
    return order === 'desc' ? -cmp : cmp;
  };

  folderItems.sort(comparator);
  fileItems.sort(comparator);

  const parentHref =
    breadcrumb.length >= 2
      ? `/?dir=${encodeURIComponent(breadcrumb[breadcrumb.length - 2].path)}`
      : breadcrumb.length === 1
        ? '/'
        : null;

  const allFolders = viewerId ? await folderService.listAllUserFolders(viewerId) : [];

  res.render('index', {
    user: req.user ?? null,
    breadcrumb,
    parentHref,
    items: [...folderItems, ...fileItems],
    sortBy,
    order,
    currentFolderId: folderId,
    currentPath,
    allFolders,
  });
};

export const redirectToDownload = async (req: Request, res: Response): Promise<void> => {
  const url = await fileService.getDownloadUrl(req.params.id, req.user?.id, false);
  res.redirect(url);
};

export const redirectToView = async (req: Request, res: Response): Promise<void> => {
  const url = await fileService.getDownloadUrl(req.params.id, req.user?.id, true);
  res.redirect(url);
};
