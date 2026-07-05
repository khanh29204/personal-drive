import { Router } from 'express';

import * as fileController from '../controllers/file.controller';
import { authMiddleware } from '../middlewares/auth.middleware';
import { optionalAuthMiddleware } from '../middlewares/optionalAuth.middleware';
import { asyncHandler } from '../utils/asyncHandler';

const router = Router();

router.get('/', optionalAuthMiddleware, asyncHandler(fileController.listFiles));
router.post('/upload-url', authMiddleware, asyncHandler(fileController.requestUploadUrl));
router.post('/:id/complete', authMiddleware, asyncHandler(fileController.completeUpload));
router.get(
  '/:id/download-url',
  optionalAuthMiddleware,
  asyncHandler(fileController.getDownloadUrl),
);
router.delete('/:id', authMiddleware, asyncHandler(fileController.deleteFile));
router.patch('/:id', authMiddleware, asyncHandler(fileController.updateFile));

export default router;
