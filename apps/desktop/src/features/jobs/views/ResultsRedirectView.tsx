import { useEffect } from "react";
import { useRouter } from "@/app/router/RouterContext";

export default function ResultsRedirectView() {
  const router = useRouter();

  useEffect(() => {
    void router.replace("/jobs?status=completed");
  }, [router]);

  return null;
}
