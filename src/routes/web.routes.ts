import { Router } from 'express';

import * as webController from '../controllers/web.controller';
import { optionalAuthMiddleware } from '../middlewares/optionalAuth.middleware';
import { asyncHandler } from '../utils/asyncHandler';

const router = Router();

router.get('/', optionalAuthMiddleware, asyncHandler(webController.renderHome));
router.get(
  '/files/:id/download',
  optionalAuthMiddleware,
  asyncHandler(webController.redirectToDownload),
);

export default router;
