import { Router } from 'express';

import * as authController from '../controllers/auth.controller';
import { asyncHandler } from '../utils/asyncHandler';

const router = Router();

router.post('/login', asyncHandler(authController.login));
router.post('/logout', authController.logout);

export default router;
