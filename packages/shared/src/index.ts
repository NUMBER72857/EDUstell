import { z } from "zod";

export const userRoleValues = [
  "PARENT",
  "CONTRIBUTOR",
  "STUDENT",
  "SCHOOL_ADMIN",
  "DONOR",
  "PLATFORM_ADMIN",
] as const;

export const milestoneStatusValues = [
  "DRAFT",
  "FUNDED",
  "PARTIALLY_PAID",
  "PAID",
  "OVERDUE",
  "CANCELLED",
] as const;

export const payoutStatusValues = [
  "DRAFT",
  "PENDING_REVIEW",
  "APPROVED",
  "REJECTED",
  "PROCESSING",
  "PAID",
  "FAILED",
  "CANCELLED",
] as const;

export const kycStatusValues = [
  "NOT_STARTED",
  "PENDING",
  "VERIFIED",
  "REJECTED",
  "EXPIRED",
] as const;

export const scholarshipStatusValues = [
  "DRAFT",
  "OPEN",
  "UNDER_REVIEW",
  "APPROVED",
  "REJECTED",
  "AWARDED",
  "CANCELLED",
] as const;

export const savingsPlanStatusValues = [
  "DRAFT",
  "ACTIVE",
  "PAUSED",
  "COMPLETED",
  "CANCELLED",
] as const;

export const vaultStatusValues = [
  "ACTIVE",
  "LOCKED",
  "COMPLETED",
  "CLOSED",
] as const;

export const contributorStatusValues = ["ACTIVE", "REVOKED"] as const;

export const contributionStatusValues = [
  "PENDING",
  "SETTLED",
  "FAILED",
  "REVERSED",
] as const;

export const verificationStatusValues = [
  "PENDING",
  "VERIFIED",
  "REJECTED",
  "EXPIRED",
] as const;

export const notificationChannelValues = [
  "EMAIL",
  "SMS",
  "PUSH",
  "WEBHOOK",
  "IN_APP",
] as const;

export const notificationStatusValues = [
  "PENDING",
  "SENT",
  "FAILED",
  "READ",
] as const;

export const auditActorTypeValues = [
  "USER",
  "SYSTEM",
  "ADMIN",
  "COMPLIANCE",
] as const;

export const UserRoleSchema = z.enum(userRoleValues);
export const MilestoneStatusSchema = z.enum(milestoneStatusValues);
export const PayoutStatusSchema = z.enum(payoutStatusValues);
export const KycStatusSchema = z.enum(kycStatusValues);
export const ScholarshipStatusSchema = z.enum(scholarshipStatusValues);
export const SavingsPlanStatusSchema = z.enum(savingsPlanStatusValues);
export const VaultStatusSchema = z.enum(vaultStatusValues);
export const ContributorStatusSchema = z.enum(contributorStatusValues);
export const ContributionStatusSchema = z.enum(contributionStatusValues);
export const VerificationStatusSchema = z.enum(verificationStatusValues);
export const NotificationChannelSchema = z.enum(notificationChannelValues);
export const NotificationStatusSchema = z.enum(notificationStatusValues);
export const AuditActorTypeSchema = z.enum(auditActorTypeValues);

const IdSchema = z.string().min(1);
const CurrencySchema = z.string().length(3).transform((value) => value.toUpperCase());
const TimestampSchema = z.string().datetime();
const NullableTimestampSchema = TimestampSchema.nullable().optional();
const OptionalDeletedAtSchema = TimestampSchema.nullable().optional();
const MoneyMinorSchema = z.string().regex(/^\d+$/);

export const BaseRecordSchema = z.object({
  id: IdSchema,
  createdAt: TimestampSchema,
  updatedAt: TimestampSchema,
});

export const SoftDeleteSchema = z.object({
  deletedAt: OptionalDeletedAtSchema,
});

export const UserSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  email: z.string().email(),
  firstName: z.string().min(1),
  lastName: z.string().min(1),
  phoneNumber: z.string().min(7).nullable().optional(),
  role: UserRoleSchema,
  isActive: z.boolean(),
  emailVerifiedAt: NullableTimestampSchema,
  onboardingCompletedAt: NullableTimestampSchema,
  mfaEnabledAt: NullableTimestampSchema,
  lastLoginAt: NullableTimestampSchema,
});

export const ChildProfileSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  ownerUserId: IdSchema,
  sponsorUserId: IdSchema.nullable().optional(),
  firstName: z.string().min(1),
  lastName: z.string().min(1),
  dateOfBirth: TimestampSchema,
  educationLevel: z.string().nullable().optional(),
  targetGraduationYear: z.number().int().nullable().optional(),
  externalReference: z.string().nullable().optional(),
});

export const SavingsPlanSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  ownerUserId: IdSchema,
  childProfileId: IdSchema,
  name: z.string().min(1),
  description: z.string().nullable().optional(),
  targetAmountMinor: MoneyMinorSchema,
  currency: CurrencySchema,
  status: SavingsPlanStatusSchema,
  startDate: NullableTimestampSchema,
  endDate: NullableTimestampSchema,
});

export const SavingsVaultSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  savingsPlanId: IdSchema,
  name: z.string().min(1),
  currency: CurrencySchema,
  targetAmountMinor: MoneyMinorSchema,
  availableBalanceMinor: MoneyMinorSchema,
  lockedBalanceMinor: MoneyMinorSchema,
  status: VaultStatusSchema,
  disbursementAuthorityUserId: IdSchema,
  onChainVaultRef: z.string().nullable().optional(),
  onChainAnchorHash: z.string().nullable().optional(),
});

export const VaultContributorSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  savingsVaultId: IdSchema,
  userId: IdSchema,
  nickname: z.string().nullable().optional(),
  permissionsNote: z.string().nullable().optional(),
  status: ContributorStatusSchema,
  canViewVault: z.boolean(),
  canTriggerPayout: z.boolean(),
});

export const MilestoneSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  savingsVaultId: IdSchema,
  schoolId: IdSchema.nullable().optional(),
  title: z.string().min(1),
  description: z.string().nullable().optional(),
  dueDate: NullableTimestampSchema,
  targetAmountMinor: MoneyMinorSchema,
  reservedAmountMinor: MoneyMinorSchema,
  status: MilestoneStatusSchema,
  beneficiaryReferenceHash: z.string().nullable().optional(),
});

export const ContributionSchema = BaseRecordSchema.extend({
  savingsVaultId: IdSchema,
  contributorUserId: IdSchema,
  amountMinor: MoneyMinorSchema,
  currency: CurrencySchema,
  status: ContributionStatusSchema,
  paymentProviderRef: z.string().nullable().optional(),
  idempotencyKey: z.string().nullable().optional(),
  settledAt: NullableTimestampSchema,
  onChainTxHash: z.string().nullable().optional(),
  ledgerReferenceHash: z.string().nullable().optional(),
});

export const PayoutRequestSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  savingsVaultId: IdSchema,
  milestoneId: IdSchema.nullable().optional(),
  schoolId: IdSchema,
  requestedByUserId: IdSchema,
  approvedByUserId: IdSchema.nullable().optional(),
  amountMinor: MoneyMinorSchema,
  currency: CurrencySchema,
  destinationAccountRef: z.string().min(1),
  destinationAccountHash: z.string().nullable().optional(),
  payoutStatus: PayoutStatusSchema,
  rejectionReason: z.string().nullable().optional(),
  approvedAt: NullableTimestampSchema,
  paidAt: NullableTimestampSchema,
  paymentReference: z.string().nullable().optional(),
  idempotencyKey: z.string().nullable().optional(),
  onChainSettlementHash: z.string().nullable().optional(),
  approvedDestinationSnapshot: z.record(z.string(), z.unknown()),
});

export const SchoolSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  name: z.string().min(1),
  countryCode: z.string().length(2).transform((value) => value.toUpperCase()),
  registrationNumber: z.string().nullable().optional(),
  website: z.string().url().nullable().optional(),
  supportEmail: z.string().email().nullable().optional(),
  payoutDestinationRef: z.string().min(1),
  payoutDestinationHash: z.string().nullable().optional(),
  approvedForPayouts: z.boolean(),
});

export const SchoolVerificationSchema = BaseRecordSchema.extend({
  schoolId: IdSchema,
  verificationStatus: VerificationStatusSchema,
  verifiedByUserId: IdSchema.nullable().optional(),
  verificationMethod: z.string().nullable().optional(),
  notes: z.string().nullable().optional(),
  evidenceUri: z.string().url().nullable().optional(),
  verifiedAt: NullableTimestampSchema,
  expiresAt: NullableTimestampSchema,
});

export const ScholarshipPoolSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  createdByUserId: IdSchema,
  name: z.string().min(1),
  description: z.string().nullable().optional(),
  totalFundingMinor: MoneyMinorSchema,
  availableFundingMinor: MoneyMinorSchema,
  currency: CurrencySchema,
  status: ScholarshipStatusSchema,
  eligibilityRules: z.record(z.string(), z.unknown()),
});

export const ScholarshipApplicationSchema = BaseRecordSchema
  .merge(SoftDeleteSchema)
  .extend({
    scholarshipPoolId: IdSchema,
    childProfileId: IdSchema,
    applicantUserId: IdSchema,
    essayText: z.string().nullable().optional(),
    supportingDocumentUri: z.string().url().nullable().optional(),
    status: ScholarshipStatusSchema,
    submittedAt: NullableTimestampSchema,
    reviewedAt: NullableTimestampSchema,
  });

export const ScholarshipAwardSchema = BaseRecordSchema.extend({
  scholarshipPoolId: IdSchema,
  applicationId: IdSchema,
  approvedByUserId: IdSchema.nullable().optional(),
  amountMinor: MoneyMinorSchema,
  currency: CurrencySchema,
  status: ScholarshipStatusSchema,
  disbursedAt: NullableTimestampSchema,
  notes: z.string().nullable().optional(),
});

export const AuditLogSchema = BaseRecordSchema.extend({
  actorUserId: IdSchema.nullable().optional(),
  actorType: AuditActorTypeSchema,
  entityType: z.string().min(1),
  entityId: IdSchema,
  action: z.string().min(1),
  requestId: z.string().nullable().optional(),
  correlationId: z.string().nullable().optional(),
  idempotencyKey: z.string().nullable().optional(),
  beforeState: z.record(z.string(), z.unknown()).nullable().optional(),
  afterState: z.record(z.string(), z.unknown()).nullable().optional(),
  metadata: z.record(z.string(), z.unknown()).nullable().optional(),
  occurredAt: TimestampSchema,
});

export const NotificationSchema = BaseRecordSchema.extend({
  userId: IdSchema,
  channel: NotificationChannelSchema,
  status: NotificationStatusSchema,
  subject: z.string().nullable().optional(),
  body: z.string().min(1),
  templateKey: z.string().nullable().optional(),
  providerRef: z.string().nullable().optional(),
  sentAt: NullableTimestampSchema,
  readAt: NullableTimestampSchema,
});

export const KycProfileSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  userId: IdSchema,
  status: KycStatusSchema,
  providerName: z.string().nullable().optional(),
  providerReference: z.string().nullable().optional(),
  legalFirstName: z.string().nullable().optional(),
  legalLastName: z.string().nullable().optional(),
  dateOfBirth: NullableTimestampSchema,
  documentCountryCode: z.string().length(2).nullable().optional(),
  documentType: z.string().nullable().optional(),
  documentNumberHash: z.string().nullable().optional(),
  addressLine1: z.string().nullable().optional(),
  addressLine2: z.string().nullable().optional(),
  city: z.string().nullable().optional(),
  stateRegion: z.string().nullable().optional(),
  postalCode: z.string().nullable().optional(),
  countryCode: z.string().length(2).nullable().optional(),
  reviewedAt: NullableTimestampSchema,
  expiresAt: NullableTimestampSchema,
});

export const WalletAccountSchema = BaseRecordSchema.merge(SoftDeleteSchema).extend({
  userId: IdSchema,
  network: z.string().min(1),
  walletAddress: z.string().min(1),
  memo: z.string().nullable().optional(),
  provider: z.string().nullable().optional(),
  isPrimary: z.boolean(),
  isCustodial: z.boolean(),
  addressHash: z.string().nullable().optional(),
  onChainReference: z.string().nullable().optional(),
});

export type UserRole = z.infer<typeof UserRoleSchema>;
export type MilestoneStatus = z.infer<typeof MilestoneStatusSchema>;
export type PayoutStatus = z.infer<typeof PayoutStatusSchema>;
export type KycStatus = z.infer<typeof KycStatusSchema>;
export type ScholarshipStatus = z.infer<typeof ScholarshipStatusSchema>;
export type SavingsPlanStatus = z.infer<typeof SavingsPlanStatusSchema>;
export type VaultStatus = z.infer<typeof VaultStatusSchema>;
export type ContributorStatus = z.infer<typeof ContributorStatusSchema>;
export type ContributionStatus = z.infer<typeof ContributionStatusSchema>;
export type VerificationStatus = z.infer<typeof VerificationStatusSchema>;
export type NotificationChannel = z.infer<typeof NotificationChannelSchema>;
export type NotificationStatus = z.infer<typeof NotificationStatusSchema>;
export type AuditActorType = z.infer<typeof AuditActorTypeSchema>;

export type BaseRecord = z.infer<typeof BaseRecordSchema>;
export type User = z.infer<typeof UserSchema>;
export type ChildProfile = z.infer<typeof ChildProfileSchema>;
export type SavingsPlan = z.infer<typeof SavingsPlanSchema>;
export type SavingsVault = z.infer<typeof SavingsVaultSchema>;
export type VaultContributor = z.infer<typeof VaultContributorSchema>;
export type Milestone = z.infer<typeof MilestoneSchema>;
export type Contribution = z.infer<typeof ContributionSchema>;
export type PayoutRequest = z.infer<typeof PayoutRequestSchema>;
export type School = z.infer<typeof SchoolSchema>;
export type SchoolVerification = z.infer<typeof SchoolVerificationSchema>;
export type ScholarshipPool = z.infer<typeof ScholarshipPoolSchema>;
export type ScholarshipApplication = z.infer<typeof ScholarshipApplicationSchema>;
export type ScholarshipAward = z.infer<typeof ScholarshipAwardSchema>;
export type AuditLog = z.infer<typeof AuditLogSchema>;
export type Notification = z.infer<typeof NotificationSchema>;
export type KycProfile = z.infer<typeof KycProfileSchema>;
export type WalletAccount = z.infer<typeof WalletAccountSchema>;

export * from "./session";
