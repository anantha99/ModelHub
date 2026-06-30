type EmptyStateProps = {
  eyebrow: string;
  title: string;
  description: string;
};

export function EmptyState({ eyebrow, title, description }: EmptyStateProps) {
  return (
    <section className="empty-state">
      <p className="eyebrow">{eyebrow}</p>
      <h2>{title}</h2>
      <p>{description}</p>
    </section>
  );
}
