export const ACCESS_COOKIE_NAME = "edustell_access";
export const REFRESH_COOKIE_NAME = "edustell_refresh";

export interface FrontendSessionUser {
  id: string;
  email: string;
  firstName: string;
  lastName: string;
  role:
    | "PARENT"
    | "CONTRIBUTOR"
    | "STUDENT"
    | "SCHOOL_ADMIN"
    | "DONOR"
    | "PLATFORM_ADMIN";
  emailVerified: boolean;
  mfaEnrolled: boolean;
}

export interface FrontendSession {
  user: FrontendSessionUser;
  authenticated: boolean;
}

export function buildSecureSessionCookieOptions(isProduction: boolean) {
  return {
    httpOnly: true,
    secure: isProduction,
    sameSite: "strict" as const,
    path: "/",
  };
}
