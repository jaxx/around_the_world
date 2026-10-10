# Reis ümber maailma

Brauseris mängitav võrgu-lauamäng 2–4 mängijale, mis põhineb Nõukogude-aegsel lauamängul «Reis ümber maailma». Mängijad heidavad kordamööda täringut ja viivad oma neli nuppu ümber X-kujulise mängulaua oma kodutee lõppu.

Mõisted on eesti keeles; koodis kasutatakse sulgudes toodud ingliskeelset nime.

## Keel

### Mängulaud

**Mängulaud** (`Board`):
X-kujuline laud, millel on 64 raja ringi, 4 × 7 koduteede ringi ja 4 × 4 pesa ringi.
_Väldi_: väli, plats

**Ring** (`Cell`):
Üks koht mängulaual, kus nupp saab seista.
_Väldi_: ruut, väli, lahter

**Rada** (`Track`):
64 ringist koosnev suletud ringtee, mida mööda kõigi värvide nupud liiguvad päripäeva.
_Väldi_: trass, ringtee

**Haru** (`Arm`):
Üks X-i neljast diagonaalsest harust; igal värvil on oma haru, mille keskmine rida on selle värvi kodutee.
_Väldi_: sakk, nurk

**Keskring** (`CenterCell`):
Üks neljast laua keskel asuvast raja ringist, mida jagavad kaks naaberharu.

**Nurgaring** (`CornerCell`):
Haru nurgas asuv raja ring vahetult enne stardiringi; siit pöörab nupp oma koduteele.

**Stardiring** (`StartCell`):
Värvi oma värviline raja ring haru nurgas, kuhu nupp pesast mängu tuleb.
_Väldi_: lähtering (see tähendab pesa ringi), alguskoht

**Pesa** (`Nest`):
Värvi neli lähteringi, kus nupud mängu alguses seisavad ja kuhu välja löödud nupp tagasi läheb.
_Väldi_: kodu, maja

**Lähtering** (`NestCell`):
Üks pesa neljast ringist.

**Kodutee** (`HomeLane`):
Haru keskmine, värvi 7 ringist koosnev rida, kuhu siseneb ainult sama värvi nupp nurgaringist ja mille lõpp on laua keskel; koduteel ei tohi nupp üle oma teise nupu liikuda ega sellele peatuda.
_Väldi_: keskmine tee, kodu, finiš

**Sihtring** (`Target`):
Koduteel sügavaim vaba ring, kuhu nupp peab täpselt jõudma; täitub lõpust alguse poole.

### Mäng

**Mängija** (`Player`):
Mängus osaleja, kellel on üks värv ja neli nuppu.

**Värv** (`Color`):
Üks neljast: punane, kollane, roheline, sinine; määrab mängija haru, pesa, stardiringi ja koduteed.

**Nupp** (`Piece`):
Mängija üks neljast mängukujust.
_Väldi_: mängunupp, nupuke, figuur

**Lõpetanud nupp** (`FinishedPiece`):
Nupp, mis on jõudnud oma sihtringile ega liigu enam.

**Torn** (`Stack`):
Sama mängija mitu nuppu ühel raja ringil (koduteel torni ei teki); iga nupp liigub ja langeb välja eraldi.

**Segaring** (`SharedCell`):
Raja ring, kus on korraga mitme värvi nupud; tekib, kui väljalöömine viib vastase tornist välja ainult ühe nupu. Seal olevad nupud ei mõjuta üksteist enne, kui keegi uus ringile saabub.

**Alustaja** (`FirstPlayer`):
Mängija, kes teeb esimese korra; selgub kõrgeima viskega, viigi korral viskavad viigistajad uuesti.

**Kord** (`Turn`):
Ühe mängija järjestikused viske(d) ja käigud enne, kui järg läheb järgmisele mängijale päripäeva.
_Väldi_: voor, käik

**Vise** (`Roll`):
Üks täringuheide, mille mängija ise algatab.
_Väldi_: viskamine, heide

**Lisavise** (`BonusRoll`):
Uus vise samas korras, mille annab iga kuue silma vise, ka siis, kui sellega käiku polnud.

**Kolm katset** (`ThreeAttempts`):
Kuni kolm viset korras, mis on lubatud siis, kui mängija ühelgi nupul pole lubatud käiku ehk kõik tema lõpetamata nupud on pesas.

**Käik** (`Move`):
Ühe nupu liigutamine viske silmade arvu võrra; kui käik on võimalik, peab mängija selle tegema ja võib valida nupu.
_Väldi_: samm, liigutus

**Väljatoomine** (`Enter`):
Käik, millega nupp liigub kuue silma viskel pesast oma stardiringile; lubatud ainult siis, kui stardiringil pole oma nuppu.

**Väljalöömine** (`Capture`):
Kui nupp lõpetab käigu ringil, kus on vastase nuppe, langeb igast vastase värvist üks nupp välja ja läheb tagasi oma pessa; ülejäänud jäävad saabujaga samale ringile.
_Väldi_: söömine, võtmine

**Tagasilugemine** (`Bounce`):
Kui viske silmi on rohkem, kui sihtringini vaja, liigub nupp ülejäänud silmade võrra tagasi sama teed pidi, vajadusel koduteelt välja rajale; ka tagasi liikudes ei tohi koduteel üle oma nupu minna.
_Väldi_: põrkamine

**Võitja** (`Winner`):
Mängija, kelle kõik neli nuppu on esimesena lõpetanud.

**Koht** (`Placement`):
Mängija järjekorranumber selle järgi, mitmendana ta kõik neli nuppu lõpetas; viimane koht jääb mängijale, kes lõpetamata jääb.

**Mängu lõpp** (`GameOver`):
Hetk, mil lõpetamata on alles ainult üks mängija; lõpetanud mängijad jäetakse kordade järjekorrast välja.

### Võrgumäng

**Tuba** (`Room`):
Mängijate kogunemiskoht, kuhu liitutakse kutselingi kaudu ja kus peetakse üks või mitu järjestikust mängu.
_Väldi_: sessioon, mäng

**Ootesaal** (`Lobby`):
Toa olek enne mängu algust: mängijad liituvad ja lahkuvad, igaüks saab automaatselt esimese vaba värvi päripäeva ja võib selle vaba värvi vastu vahetada.

**Võõrustaja** (`Host`):
Toa looja või tema ootesaalist lahkumisel järgmine mängija; alustab mängu, kui toas on 2–4 mängijat.

**Külaline** (`Guest`):
Ilma kontota mängija, keda tuntakse ainult hüüdnime järgi.

**Kutselink** (`InviteLink`):
Link, mille kaudu külaline tuppa liitub.

**Käiguaeg** (`TurnTimer`):
30-sekundiline aeg, mille jooksul mängija peab viskama või käima; selle möödumisel viskab server tema eest või teeb esimese lubatud käigu.

**Eemal** (`Away`):
Mängija olek pärast kolme järjestikust käiguaja möödumist või kui ühendus on katkenud kauem kui 10 sekundit; server mängib tema eest. Lõpeb, kui mängija ise viskab või käib või uuesti ühendub.

**Uus mäng** (`Rematch`):
Võõrustaja valik pärast mängu lõppu viia tuba samade mängijatega tagasi ootesaali.
