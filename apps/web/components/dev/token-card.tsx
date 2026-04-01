"use client";

import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardAction } from "@/components/ui/card";
import { Button } from "@/components/ui/button";

export function TokenCard({
  title,
  data,
  copyValue,
}: {
  title: string;
  data: string | Record<string, unknown>;
  copyValue?: string;
}) {
  const [copied, setCopied] = useState(false);
  const textToCopy = copyValue ?? (typeof data === "string" ? data : JSON.stringify(data, null, 2));

  function handleCopy() {
    navigator.clipboard.writeText(textToCopy);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardAction>
          <Button variant="outline" size="sm" onClick={handleCopy}>
            {copied ? "Copied!" : "Copy"}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        <pre className="bg-muted overflow-x-auto rounded-lg p-3 text-xs">
          {typeof data === "string" ? data : JSON.stringify(data, null, 2)}
        </pre>
      </CardContent>
    </Card>
  );
}
