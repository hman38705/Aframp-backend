/**
 * Next.js Middleware
 * Client-side navigation guards for KYB/KYC enforcement and route protection
 */

import { NextResponse, type NextRequest } from 'next/server';
import createMiddleware from 'next-intl/middleware';
import { locales, defaultLocale } from './config/locales';

// ============================================================================
// Route Protection Configuration
// ============================================================================

const PUBLIC_ROUTES = [
  '/auth/login',
  '/auth/register',
  '/auth/forgot-password',
  '/auth/reset-password',
];

const KYC_REQUIRED_ROUTES = [
  '/dashboard',
  '/wallets',
  '/transactions',
  '/exchange',
  '/send',
  '/receive',
];

const KYB_REQUIRED_ROUTES = [
  '/partner',
  '/merchant',
  '/api-keys',
  '/webhooks',
];

// ============================================================================
// Internationalization Middleware
// ============================================================================

const intlMiddleware = createMiddleware({
  locales,
  defaultLocale,
  localePrefix: 'as-needed',
});

// ============================================================================
// Content Security Policy
// ============================================================================

function buildCspHeader(nonce: string): string {
  const isDev = process.env.NODE_ENV === 'development';
  const apiUrl = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';

  const csp = `
    default-src 'self';
    script-src 'self' 'nonce-${nonce}'${isDev ? " 'unsafe-eval'" : ''};
    style-src 'self' 'unsafe-inline';
    img-src 'self' data: blob:;
    font-src 'self' data:;
    connect-src 'self' ${apiUrl};
    frame-ancestors 'none';
    base-uri 'self';
    form-action 'self';
    object-src 'none';
    ${isDev ? '' : 'upgrade-insecure-requests;'}
  `;

  return csp.replace(/\s{2,}/g, ' ').trim();
}

function withCsp(response: NextResponse, csp: string): NextResponse {
  response.headers.set('Content-Security-Policy', csp);
  return response;
}

// ============================================================================
// Main Middleware
// ============================================================================

export default async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const nonce = Buffer.from(crypto.randomUUID()).toString('base64');
  const csp = buildCspHeader(nonce);

  // Apply internationalization
  const response = intlMiddleware(request);

  // Extract locale from pathname
  const pathnameLocale = locales.find(
    (locale) => pathname.startsWith(`/${locale}/`) || pathname === `/${locale}`
  );
  const pathWithoutLocale = pathnameLocale
    ? pathname.slice(`/${pathnameLocale}`.length) || '/'
    : pathname;

  // Check authentication
  const accessToken = request.cookies.get('aframp_access_token')?.value;
  const isAuthenticated = !!accessToken;

  // Public routes - allow access
  if (PUBLIC_ROUTES.some((route) => pathWithoutLocale.startsWith(route))) {
    return withCsp(response, csp);
  }

  // Protected routes - require authentication
  if (!isAuthenticated) {
    const loginUrl = new URL(
      pathnameLocale ? `/${pathnameLocale}/auth/login` : '/auth/login',
      request.url
    );
    loginUrl.searchParams.set('redirect', pathname);
    return withCsp(NextResponse.redirect(loginUrl), csp);
  }

  // KYC enforcement
  if (KYC_REQUIRED_ROUTES.some((route) => pathWithoutLocale.startsWith(route))) {
    const kycStatus = request.cookies.get('aframp_kyc_status')?.value;

    if (kycStatus !== 'approved') {
      const kycUrl = new URL(
        pathnameLocale ? `/${pathnameLocale}/onboarding/kyc` : '/onboarding/kyc',
        request.url
      );
      return withCsp(NextResponse.redirect(kycUrl), csp);
    }
  }

  // KYB enforcement for partner/merchant routes
  if (KYB_REQUIRED_ROUTES.some((route) => pathWithoutLocale.startsWith(route))) {
    const kybStatus = request.cookies.get('aframp_kyb_status')?.value;

    if (kybStatus !== 'approved') {
      const kybUrl = new URL(
        pathnameLocale ? `/${pathnameLocale}/onboarding/kyb` : '/onboarding/kyb',
        request.url
      );
      return withCsp(NextResponse.redirect(kybUrl), csp);
    }
  }

  return withCsp(response, csp);
}

export const config = {
  matcher: [
    // Match all pathnames except for
    // - … if they start with `/api`, `/_next` or `/_vercel`
    // - … the ones containing a dot (e.g. `favicon.ico`)
    '/((?!api|_next|_vercel|.*\\..*).*)',
  ],
};
