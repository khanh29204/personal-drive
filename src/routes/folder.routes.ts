import { Router } from 'express';

import * as folderController from '../controllers/folder.controller';
import { authMiddleware } from '../middlewares/auth.middleware';
import { optionalAuthMiddleware } from '../middlewares/optionalAuth.middleware';
import { asyncHandler } from '../utils/asyncHandler';

const router = Router();

router.get('/', optionalAuthMiddleware, asyncHandler(folderController.listFolders));
router.post('/', authMiddleware, asyncHandler(folderController.createFolder));
router.patch('/:id', authMiddleware, asyncHandler(folderController.updateFolder));
router.delete('/:id', authMiddleware, asyncHandler(folderController.deleteFolder));

export default router;
