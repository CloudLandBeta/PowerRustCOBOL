      *> CUS-REQ: what the Customers screen sends to CUSTOMER-SAVE. The browser sends VALUES only.
      *> CRQ-ID   0 = a new customer; otherwise the customer to change.
      *> CRQ-DOC  CPF (11 digits) or CNPJ (14 digits), digits only.
      *> CRQ-CONSENT-EMAIL / CRQ-CONSENT-WA  S = the customer agreed to be contacted on that channel, N = not.
      *> CRQ-OPERATOR  the operator's name, typed when the session opened (there is no login): it goes to the audit trail.
       01 CUS-REQ.
          05 CRQ-ID                              PIC    9(8).
          05 CRQ-NAME                            PIC    X(60).
          05 CRQ-DOC                             PIC    X(14).
          05 CRQ-ADDRESS                         PIC    X(80).
          05 CRQ-PHONE                           PIC    X(20).
          05 CRQ-WHATSAPP                        PIC    X(20).
          05 CRQ-EMAIL                           PIC    X(60).
          05 CRQ-CONSENT-EMAIL                   PIC    X(1).
          05 CRQ-CONSENT-WA                      PIC    X(1).
          05 CRQ-OPERATOR                        PIC    X(30).
