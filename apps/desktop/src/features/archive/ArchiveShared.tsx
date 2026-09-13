import type { ReactNode } from "react";
import { ArrowLeftRegular, WarningRegular } from "@fluentui/react-icons";
import { Card } from "@qzip/ui";
import type { ArchiveRisk } from "../../contracts/archive";
import { useI18n } from "../../lib/i18n";

export function RiskNotice({ risks, accepted, onAccepted }: {
  risks: ArchiveRisk[];
  accepted: boolean;
  onAccepted: (value: boolean) => void;
}) {
  const { text } = useI18n();
  if (!risks.length) return null;
  const mayContinue = risks.some((risk) => risk.overridable);
  return (
    <aside className="qzip-risk-notice">
      <WarningRegular fontSize={22} />
      <div>
        <strong>{text("安全检查提示", "Security notice")}</strong>
        <p>{risks.map((risk) => risk.message).join("；")}</p>
        {mayContinue ? (
          <label>
            <input type="checkbox" checked={accepted} onChange={(event) => onAccepted(event.target.checked)} />
            {text("我已了解本次风险并继续", "I understand the risk and want to continue")}
          </label>
        ) : null}
      </div>
    </aside>
  );
}

export function DetailWorkspace({ title, onBack, className, children }: {
  title: string;
  onBack: () => void;
  className?: string;
  children: ReactNode;
}) {
  const { text } = useI18n();
  return (
    <section className={`qzip-detail-page ${className ?? ""}`}>
      <Card className="qzip-detail-panel">
        <header className="qzip-detail-panel__header">
          <button type="button" aria-label={text("返回首页", "Back to home")} onClick={onBack}><ArrowLeftRegular fontSize={26} /></button>
          <h1>{title}</h1>
        </header>
        {children}
      </Card>
    </section>
  );
}

export function FormRow({ label, children }: { label: string; children: ReactNode }) {
  return <div className="qzip-form-row"><strong>{label}</strong><div>{children}</div></div>;
}

export function Stat({ label, value }: { label: string; value: string }) {
  return <div className="qzip-stat"><span>{label}</span><strong>{value}</strong></div>;
}

export function Empty({ icon, text }: { icon: ReactNode; text: string }) {
  return <div className="qzip-work-empty">{icon}<p>{text}</p></div>;
}
